"""Bounded constant-argument specialization of Python function methods."""

import struct
from collections import deque
from dataclasses import dataclass

from kirin import ir, types
from kirin.rewrite import Walk, WrapConst, Call2Invoke
from kirin.analysis import const
from kirin.dialects import func
from kirin.passes.abc import Pass
from kirin.passes.fold import Fold
from kirin.rewrite.abc import RewriteResult
from kirin.dialects.ilist import IList
from kirin.dialects.py.constant import Constant
from kirin.rewrite.specialize_invoke import SpecializeInvoke


def _constant_key(value: object) -> tuple | None:
    """None means dynamic; keys never invoke arbitrary user equality/hash."""
    kind = type(value)
    if kind in (type(None), bool, int, str, bytes):
        return (kind, value)
    if type(value) is float:
        return (float, struct.pack("!d", value))
    if type(value) is complex:
        return (complex, struct.pack("!dd", value.real, value.imag))
    if type(value) is range:
        return (range, value.start, value.stop, value.step)
    if type(value) is tuple:
        items = tuple(_constant_key(item) for item in value)
        if any(item is None for item in items):
            return None
        return (tuple, items)
    if type(value) is frozenset:
        items = tuple(_constant_key(item) for item in value)
        if any(item is None for item in items):
            return None
        return (frozenset, frozenset(items))
    if type(value) is IList:
        items = tuple(_constant_key(item) for item in value)
        if any(item is None for item in items):
            return None
        return (IList, id(value.elem), items)
    return None


@dataclass
class Specialize(Pass):
    """Specialize statically reachable func methods on supported constants.

    Each invocation starts from one method and owns a fresh cache.
    Generic methods retain their signatures; visited bodies may be folded.
    A zero budget disables specialization.
    """

    max_specializations: int = 32

    def __post_init__(self):
        if self.max_specializations < 0:
            raise ValueError("max_specializations must be nonnegative")

    def unsafe_run(self, mt: ir.Method) -> RewriteResult:
        # Working state belongs to one invocation, even when the pass is reused.
        self.cache: dict[tuple, tuple[ir.Method, tuple[int, ...]]] = {}
        self.counts: dict[ir.Method, int] = {}
        self.generated: set[ir.Method] = set()
        self.pending: deque[ir.Method] = deque()
        self.seen: set[ir.Method] = set()
        self.names: set[str | None] = set()
        self.ordinal = 0
        if self.max_specializations == 0:
            return RewriteResult()
        return self._run(mt)

    def _discover(self, root: ir.Method):
        # Reserve existing names without CallGraph's recursive descent. Also
        # inspect method constants, since Call2Invoke may resolve them later.
        pending = [root]
        seen: set[ir.Method] = set()
        while pending:
            mt = pending.pop()
            if mt in seen:
                continue
            seen.add(mt)
            self.names.add(mt.sym_name)
            if isinstance(mt.code, func.Function):
                self.names.add(mt.code.sym_name)
            for stmt in mt.code.walk():
                if isinstance(stmt, func.Invoke):
                    pending.append(stmt.callee)
                elif (
                    isinstance(stmt, Constant)
                    and isinstance(stmt.value, ir.PyAttr)
                    and isinstance(stmt.value.data, ir.Method)
                ):
                    pending.append(stmt.value.data)

    def _analyze(self, mt: ir.Method) -> const.Frame:
        analysis = const.Propagate(mt.dialects)
        if self.no_raise:
            frame, _ = analysis.run_no_raise(mt)
        else:
            frame, _ = analysis.run(mt)
        return frame

    def _run(self, root: ir.Method) -> RewriteResult:
        self._discover(root)
        self.pending.append(root)
        result = RewriteResult()
        while self.pending:
            mt = self.pending.popleft()
            if mt in self.seen:
                continue
            self.seen.add(mt)
            old_callees = self._callees(mt)
            frame = self._analyze(mt)
            result = Walk(WrapConst(frame)).rewrite(mt.code).join(result)
            # Literal method references remain sound even when recursive
            # analysis failed and returned an empty frame.
            for stmt in mt.code.walk():
                if isinstance(stmt, Constant) and isinstance(stmt.value, ir.PyAttr):
                    stmt.result.hints["const"] = const.Value(stmt.value.data)
            result = Walk(Call2Invoke()).rewrite(mt.code).join(result)
            result = (
                Walk(SpecializeInvoke(self.materialize)).rewrite(mt.code).join(result)
            )
            result = Fold(mt.dialects, no_raise=self.no_raise)(mt).join(result)
            new_callees = self._callees(mt)
            for callee in set(old_callees) - set(new_callees):
                callee.backedges.discard(mt)
            mt.update_backedges()
            self.pending.extend(new_callees)
        return result

    @staticmethod
    def _callees(mt: ir.Method) -> tuple[ir.Method, ...]:
        # Stable traversal makes names and budget allocation reproducible.
        return tuple(
            dict.fromkeys(
                stmt.callee for stmt in mt.code.walk() if isinstance(stmt, func.Invoke)
            )
        )

    def materialize(
        self, original: ir.Method, args: tuple[const.Result, ...]
    ) -> tuple[ir.Method, tuple[int, ...]] | None:
        """Reuse a specialization or create and schedule one within the budget."""
        if (
            not isinstance(original.code, func.Function)
            or original in self.generated
            or Constant.dialect not in original.dialects
        ):
            return None
        if len(args) != len(original.args):
            return None
        keys = tuple(
            _constant_key(arg.data) if isinstance(arg, const.Value) else None
            for arg in args
        )
        if all(key is None for key in keys):
            return None
        key = (original, keys)
        if key in self.cache:
            return self.cache[key]
        count = self.counts.get(original, 0)
        if count >= self.max_specializations:
            return None
        original_entry = original.callable_region.blocks[0]
        if original_entry.first_stmt is None or any(
            original_entry in stmt.successors for stmt in original.code.walk()
        ):
            # Entry backedges would require adapting their argument lists too.
            return None
        retained = tuple(i for i, key in enumerate(keys) if key is None)
        clone = self._build_specialized_method(original, args, retained)
        self.counts[original] = count + 1
        self.generated.add(clone)
        self.cache[key] = (clone, retained)
        self.pending.append(clone)
        return clone, retained

    def _build_specialized_method(
        self,
        original: ir.Method,
        args: tuple[const.Result, ...],
        retained: tuple[int, ...],
    ) -> ir.Method:
        """Build and verify a clone with constants replacing omitted parameters."""
        clone = original.similar()
        code = clone.code
        assert isinstance(code, func.Function)
        entry = clone.callable_region.blocks[0]
        first = entry.first_stmt
        assert first is not None
        # Hints belong to the analysis context, not the original syntax.
        for stmt in code.walk():
            for value in stmt.results:
                value.hints.pop("const", None)
        # Self inside the original body denotes the original method, including
        # recursive calls with its original arity and accesses to captures.
        if entry.args[0].uses:
            method_const = Constant(original)
            method_const.insert_before(first)
            entry.args[0].replace_by(method_const.result)
        parameters = tuple(entry.args[1:])
        for i in reversed(range(len(args))):
            if i in retained:
                continue
            arg = args[i]
            assert isinstance(arg, const.Value)
            constant = Constant(arg.data)
            constant.result.name = parameters[i].name
            constant.insert_before(first)
            parameters[i].replace_by(constant.result)
            entry.args.delete(parameters[i])
        code.signature = func.Signature(
            tuple(code.signature.inputs[i] for i in retained), code.signature.output
        )
        code.slots = tuple(code.slots[i] for i in retained)
        entry.args[0].type = types.FunctionType(
            code.signature.inputs, code.signature.output
        )
        while True:
            self.ordinal += 1
            name = f"{original.sym_name}_specialized_{self.ordinal}"
            if name not in self.names:
                break
        self.names.add(name)
        code.sym_name = clone.sym_name = name
        clone.nargs = len(entry.args)
        if original.arg_names is not None:
            clone.arg_names = [original.arg_names[0]] + [
                original.arg_names[i + 1] for i in retained
            ]
        clone.inferred = False
        clone.verify()
        return clone
