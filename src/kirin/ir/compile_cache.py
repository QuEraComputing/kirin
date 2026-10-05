"""On-disk cache of compiled kernels, enabled by default.

Dialect group decorators cache kernels in `__kirincache__` beside the Python
source file, loading saved kernels instead of running the passes again when the
fingerprint matches. Set `KIRIN_COMPILE_CACHE_DIR` to override the directory, or
set it to `FALSE` (case-insensitive) to disable caching. Unset or empty values
use the default directory. Without an override, functions
without a real source file are compiled without caching. Unwritable cache
directories also fall back to normal compilation.

A kernel is looked up by a key over

- its lowered IR, which holds the values of the globals, closure variables and
  attributes the lowering read,
- the keys of the kernels its lowered IR refers to (a kernel that did not come
  from a decorator, such as a closure returned by another kernel, is keyed by its
  own IR and captured values),
- the dialect group, the source of its `run_pass` generator and the decorator's
  options,
- the file and line the function starts at, which its source locations refer to,
- the Python version and the versions of all installed distributions.

Values are described by their content: built-in scalars, tuples, lists, frozensets,
classes, kernels, and dataclasses (which include Kirin's attributes) field by
field. A kernel whose lowered IR holds any other value, or whose compiled code
pickle cannot save or load, is compiled as usual.

!!! warning
    Loading a pickle can run arbitrary code, so only point the cache at a
    directory you trust.
"""

from __future__ import annotations

import io
import os
import sys
import pickle
import hashlib
import inspect
import weakref
import dataclasses
from typing import TYPE_CHECKING, Any, Iterable
from pathlib import Path
from importlib import metadata

from kirin.ir.method import Method

if TYPE_CHECKING:
    from kirin.ir.group import DialectGroup
    from kirin.ir.nodes.stmt import Statement

ENV_VAR = "KIRIN_COMPILE_CACHE_DIR"
"""Override the source-local cache directory; `FALSE` disables caching (case-insensitive)."""

FORMAT = "1"
"""Part of every key; bump it when the key or the file layout changes."""

_keys: dict[int, str] = {}
"""Keys of the kernels compiled or loaded with the cache, by `id` of the method."""
_kernels: weakref.WeakValueDictionary[str, Method] = weakref.WeakValueDictionary()
"""The live kernel of each key, to link saved kernels to."""
_toolchain: str | None = None
"""The Python version and installed distributions, read once per process since
reading them takes tens of milliseconds."""


def directory(source_file: str) -> Path | None:
    """Choose an override or the cache beside the function's source file."""
    path = os.environ.get(ENV_VAR)
    if path:
        return None if path.casefold() == "false" else Path(path)
    if not source_file or source_file.startswith("<"):
        return None
    try:
        source = Path(source_file).absolute()
        return source.parent / "__kirincache__" if source.is_file() else None
    except OSError:
        return None


@dataclasses.dataclass(frozen=True)
class Fingerprint:
    key: str
    callees: tuple[Method, ...]
    """Kernels the lowered IR refers to, directly or through kernels without a key."""


def fingerprint(
    group: DialectGroup, code: Statement, args: tuple, options: dict[str, Any]
) -> Fingerprint | None:
    """Key of a kernel lowered to `code`, or `None` if it cannot be cached."""
    global _toolchain
    state = _Keying()
    try:
        if _toolchain is None:
            dists = sorted(
                f"{d.metadata['Name']}=={d.version}" for d in metadata.distributions()
            )
            version = f"python{sys.version_info.major}.{sys.version_info.minor}"
            _toolchain = ";".join((version, *dists))
        names = ",".join(sorted(dialect.name for dialect in group.data))
        gen = group.run_pass_gen
        passes = inspect.getsource(gen) if gen is not None else ""
        origin = next(
            (
                (stmt.source.file, stmt.source.lineno_begin)
                for stmt in code.walk()
                if stmt.source is not None
            ),
            None,
        )
        text = "\n".join(
            (
                FORMAT,
                _toolchain,
                f"{names};{passes}",
                _value_text((args, sorted(options.items())), state),
                repr(origin),
                _ir_text(code, state),
            )
        )
    except Exception:
        # including from user code: compile as usual instead
        return None
    callees = tuple({id(callee): callee for callee in state.callees}.values())
    return Fingerprint(hashlib.sha256(text.encode()).hexdigest(), callees)


def remember(mt: Method, key: str) -> None:
    if id(mt) not in _keys:
        weakref.finalize(mt, _keys.pop, id(mt), None)
    _keys[id(mt)] = key
    _kernels[key] = mt


def forget(mt: Method) -> None:
    """Drop the keys of `mt` and of every kernel that calls it, transitively."""
    stack, seen = [mt], set()
    while stack:
        method = stack.pop()
        if id(method) in seen:
            continue
        seen.add(id(method))
        key = _keys.pop(id(method), None)
        if key is not None and _kernels.get(key) is method:
            del _kernels[key]
        stack.extend(method.backedges)


def load(
    root: Path, name: str, key: str, mt: Method, callees: Iterable[Method]
) -> bool:
    """Replace the lowered code of `mt` by its saved compiled code, if there is one."""
    path = _path(root, name, key)
    try:
        with path.open("rb") as file:
            inferred, code = _Unpickler(file, mt, callees).load()
    except Exception:
        # missing, unreadable, or refers to a kernel that is gone: compile it
        return False
    mt.code = code
    mt.inferred = inferred
    mt.update_backedges()
    return True


def store(root: Path, name: str, key: str, mt: Method) -> None:
    buffer = io.BytesIO()
    try:
        _Pickler(buffer, mt).dump((mt.inferred, mt.code))
    except Exception:
        # e.g. a lambda, or a kernel without a key, in the compiled code: it is
        # compiled again next time
        return
    path = _path(root, name, key)
    tmp = path.with_name(f"{path.name}.{os.getpid()}.tmp")
    try:
        root.mkdir(parents=True, exist_ok=True)
        tmp.write_bytes(buffer.getvalue())
        os.replace(tmp, path)
    except OSError:
        try:
            tmp.unlink(missing_ok=True)
        except OSError:
            pass  # read-only/inaccessible source directories must still compile


def _path(root: Path, name: str, key: str) -> Path:
    safe = "".join(c if c.isalnum() or c in "_-." else "_" for c in name)[:80]
    return root / f"{safe}-{key}.pickle"


# Keys


class _Unkeyable(Exception):
    """A value in the lowered IR has no reliable fingerprint."""


@dataclasses.dataclass
class _Keying:
    """State of one key computation."""

    callees: list[Method] = dataclasses.field(default_factory=list)
    ir_keys: dict[int, str | None] = dataclasses.field(default_factory=dict)
    """Keys of the methods without one, by `id`; `None` while being computed."""


def _ir_text(code: Statement, state: _Keying) -> str:
    # SSA values and blocks are numbered in visiting order, so the text does not
    # depend on names or object identities.
    values: dict[int, int] = {}
    blocks: dict[int, int] = {}
    lines: list[str] = []

    def number(table: dict[int, int], obj: object) -> int:
        return table.setdefault(id(obj), len(table))

    def visit(stmt: Statement) -> None:
        cls = type(stmt)
        parts = [
            f"{cls.__module__}.{cls.__qualname__}",
            ",".join(str(number(values, arg)) for arg in stmt.args),
            ",".join(
                f"{number(values, result)}:{_value_text(result.type, state)}"
                for result in stmt.results
            ),
            ",".join(
                f"{name}={_value_text(value, state)}"
                for name, value in sorted(stmt.attributes.items())
            ),
            ",".join(str(number(blocks, block)) for block in stmt.successors),
        ]
        lines.append(" ".join(parts))
        for region in stmt.regions:
            lines.append("{")
            for block in region.blocks:
                block_args = ",".join(
                    f"{number(values, arg)}:{_value_text(arg.type, state)}"
                    for arg in block.args
                )
                lines.append(f"^{number(blocks, block)}({block_args})")
                for child in block.stmts:
                    visit(child)
            lines.append("}")

    visit(code)
    return "\n".join(lines)


def _value_text(value: Any, state: _Keying) -> str:
    """A text for `value` that differs whenever its content differs and is the same
    in every process; raises `_Unkeyable` for anything it cannot describe that way.
    """
    cls = type(value)
    # exact types: a subclass may override `__repr__`
    if value is None or cls in (bool, int, float, complex, str, bytes):
        return repr(value)
    if cls in (tuple, list):
        items = ",".join(_value_text(item, state) for item in value)
        return f"{cls.__name__}({items})"
    if cls is frozenset:
        items = ",".join(sorted(_value_text(item, state) for item in value))
        return f"frozenset{{{items}}}"
    if isinstance(value, type):
        return f"type:{value.__module__}.{value.__qualname__}"
    if isinstance(value, Method):
        state.callees.append(value)
        key = _keys.get(id(value))
        if key is None:
            # not from a decorator, e.g. a closure returned by a kernel: key it by
            # its own IR and captured values
            if id(value) in state.ir_keys:
                key = state.ir_keys[id(value)]
                if key is None:
                    raise _Unkeyable  # refers to itself
            else:
                state.ir_keys[id(value)] = None
                text = _ir_text(value.code, state) + _value_text(value.fields, state)
                key = hashlib.sha256(text.encode()).hexdigest()
                state.ir_keys[id(value)] = key
        return f"Method:{key}"
    if dataclasses.is_dataclass(value):
        names = [field.name for field in dataclasses.fields(value)]
        if set(getattr(value, "__dict__", ())) - set(names):
            raise _Unkeyable  # state outside the fields, e.g. set from an InitVar
        fields = ",".join(
            f"{name}={_value_text(getattr(value, name), state)}" for name in names
        )
        return f"{cls.__module__}.{cls.__qualname__}({fields})"
    raise _Unkeyable


# Storage


class _Pickler(pickle.Pickler):
    """Saves live kernels and dialect groups by key or name, and rebuilds singletons
    through their constructors so that they load as the same objects."""

    def __init__(self, file, root: Method):
        from kirin.ir.group import DialectGroup
        from kirin.lattice.abc import SingletonMeta
        from kirin.interp.undefined import UndefinedMeta

        super().__init__(file, protocol=pickle.HIGHEST_PROTOCOL)
        self.root = root
        self.group_cls = DialectGroup
        self.singleton_metas = SingletonMeta, UndefinedMeta

    def persistent_id(self, obj):
        if isinstance(obj, Method):
            if obj is self.root:
                return ("self", None)
            if (key := _keys.get(id(obj))) is not None:
                return ("kernel", key)
            if obj.py_func is not None:
                raise pickle.PicklingError(f"calls {obj.sym_name}, which has no key")
            return None  # e.g. a closure made by a pass, saved by value
        if isinstance(obj, self.group_cls):
            return ("group", tuple(sorted(dialect.name for dialect in obj.data)))
        return None

    def reducer_override(self, obj):
        cls = type(obj)
        if isinstance(cls, self.singleton_metas):
            # e.g. `types.Any`, which is compared with `is`
            return cls, ()
        if cls is Method:
            names = (
                "dialects",
                "code",
                "nargs",
                "sym_name",
                "arg_names",
                "fields",
                "file",
                "lineno_begin",
                "inferred",
            )
            state = {name: getattr(obj, name) for name in names}
            state["mod"] = obj.mod.__name__ if obj.mod is not None else None
            return _method, (), state, None, None, _fill_method
        return NotImplemented


class _Unpickler(pickle.Unpickler):

    def __init__(self, file, root: Method, callees: Iterable[Method]):
        super().__init__(file)
        self.root = root
        self.kernels = {_keys[id(m)]: m for m in callees if id(m) in _keys}

    def persistent_load(self, pid):
        kind, name = pid
        if kind == "self":
            return self.root
        if kind == "kernel":
            mt = self.kernels.get(name) or _kernels.get(name)
            if mt is None:
                raise pickle.UnpicklingError(f"no live kernel with key {name}")
            return mt
        for mt in (self.root, *self.kernels.values(), *_kernels.values()):
            if tuple(sorted(dialect.name for dialect in mt.dialects.data)) == name:
                return mt.dialects
        raise pickle.UnpicklingError(f"no live dialect group {name}")


# Module-level so that pickle can find them by name.


def _method() -> Method:
    mt = Method.__new__(Method)
    mt.backedges = weakref.WeakSet()
    mt.run_passes = None
    return mt


def _fill_method(mt: Method, state: dict[str, Any]) -> None:
    state["mod"] = sys.modules.get(state["mod"]) if state["mod"] else None
    for name, value in state.items():
        setattr(mt, name, value)
    mt.py_func = None
    mt.update_backedges()
