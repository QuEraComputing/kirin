import inspect
from collections.abc import Iterable
from dataclasses import dataclass
from types import ModuleType
from typing import (
    TYPE_CHECKING,
    Callable,
    Concatenate,
    Generic,
    ParamSpec,
    TypeVar,
    Union,
    overload,
)

from kirin.exceptions import CompilerError
from kirin.ir.method import Method

if TYPE_CHECKING:
    from kirin.codegen.impl import (
        Signature as CodegenSignature,
        StatementImpl as CodegenImpl,
    )
    from kirin.interp.impl import Signature, StatementImpl as InterpImpl
    from kirin.ir.dialect import Dialect
    from kirin.lowering.dialect import FromPythonAST


@dataclass
class Registry:
    parent: "DialectGroup"

    def lowering(self, keys: Iterable[str]):
        ret: dict[str, "FromPythonAST"] = {}
        from_ast = None
        for dialect in self.parent.data:
            for key in keys:
                if key in dialect.lowering:
                    from_ast = dialect.lowering[key]
                    break

            if from_ast is None:
                msg = ",".join(keys)
                raise KeyError(f"Lowering not found for {msg}")

            for name in from_ast.names:
                if name in ret:
                    raise KeyError(f"Lowering {name} already exists")

                ret[name] = from_ast
        return ret

    def interpreter(self, keys: Iterable[str]):
        """select the dialect interpreter for the given key.

        ### Args
        - `keys: Iterable[str]` the keys to search for in the dialects

        ### Returns
        - `dict["Signature", "ImplFunction"]` a map of dialects to their interpreters
        """
        from kirin.interp.impl import MethodImpl

        ret: dict["Signature", "InterpImpl"] = {}
        fallback: dict["Dialect", "InterpImpl"] = {}
        for dialect in self.parent.data:
            dialect_interp = None
            for key in keys:
                if key in dialect.interps:
                    dialect_interp = dialect.interps[key]
                    if dialect not in fallback:  # use the first fallback
                        fallback[dialect] = dialect_interp.fallback

                    for sig, func in dialect_interp.table.items():
                        if sig not in ret:
                            ret[sig] = MethodImpl(dialect_interp, func)

            if dialect not in fallback:
                msg = ",".join(keys)
                raise KeyError(f"Interpreter of {dialect.name} not found for {msg}")
        return ret, fallback

    def codegen(self, keys: Iterable[str]):
        from kirin.codegen.impl import MethodImpl

        ret: dict["CodegenSignature", "CodegenImpl"] = {}
        for dialect in self.parent.data:
            dialect_codegen = None
            for key in keys:
                if key in dialect.codegen:
                    dialect_codegen = dialect.codegen[key]
                    break

            # not found, just skip
            if dialect_codegen is None:
                continue

            for key, func in dialect_codegen.table.items():
                ret[key] = MethodImpl(dialect_codegen, func)
        return ret


PassParams = ParamSpec("PassParams")
RunPass = Callable[Concatenate[Method, PassParams], None]
RunPassGen = Callable[["DialectGroup"], RunPass[PassParams]]


@dataclass(init=False)
class DialectGroup(Generic[PassParams]):
    # method wrapper params
    Param = ParamSpec("Param")
    RetType = TypeVar("RetType")
    MethodTransform = Callable[[Callable[Param, RetType]], Method[Param, RetType]]

    data: frozenset["Dialect"]
    # NOTE: this is used to create new dialect groups from existing one
    run_pass_gen: RunPassGen[PassParams] | None = None
    run_pass: RunPass[PassParams] | None = None

    def __init__(
        self,
        dialects: Iterable[Union["Dialect", ModuleType]],
        run_pass: RunPassGen[PassParams] | None = None,
    ):
        def identity(code: Method):
            pass

        self.data = frozenset(self.map_module(dialect) for dialect in dialects)
        if run_pass is None:
            self.run_pass_gen = None
            self.run_pass = None
        else:
            self.run_pass_gen = run_pass
            self.run_pass = run_pass(self)

    def __iter__(self):
        return iter(self.data)

    @staticmethod
    def map_module(dialect):
        if isinstance(dialect, ModuleType):
            return getattr(dialect, "dialect")
        return dialect

    def add(self, dialect: Union["Dialect", ModuleType]) -> "DialectGroup":
        return self.union([dialect])

    def union(self, dialect: Iterable[Union["Dialect", ModuleType]]) -> "DialectGroup":
        return DialectGroup(
            dialects=self.data.union(frozenset(self.map_module(d) for d in dialect)),
            run_pass=self.run_pass_gen,  # pass the run_pass_gen function
        )

    @property
    def registry(self):
        return Registry(self)

    @overload
    def __call__(
        self,
        py_func: Callable[Param, RetType],
        *args: PassParams.args,
        **options: PassParams.kwargs,
    ) -> Method[Param, RetType]: ...

    @overload
    def __call__(
        self,
        py_func: None = None,
        *args: PassParams.args,
        **options: PassParams.kwargs,
    ) -> MethodTransform[Param, RetType]: ...

    def __call__(
        self,
        py_func: Callable[Param, RetType] | None = None,
        *args: PassParams.args,
        **options: PassParams.kwargs,
    ) -> Method[Param, RetType] | MethodTransform[Param, RetType]:
        from kirin.lowering import Lowering

        emit_ir = Lowering(self)

        def wrapper(py_func: Callable) -> Method:
            if py_func.__name__ == "<lambda>":
                raise ValueError("Cannot compile lambda functions")

            frame = inspect.currentframe()
            if (
                frame
                and frame.f_back is not None
                and frame.f_back.f_back is not None
                and py_func.__name__ in frame.f_back.f_back.f_locals
            ):
                raise CompilerError(
                    f"overwriting function definition of `{py_func.__name__}`"
                )
            code = emit_ir.run(py_func)
            mt = Method(
                mod=inspect.getmodule(py_func),
                py_func=py_func,
                sym_name=py_func.__name__,
                arg_names=["#self#"] + inspect.getfullargspec(py_func).args,
                dialects=self,
                code=code,
            )
            if doc := inspect.getdoc(py_func):
                mt.__doc__ = doc

            if self.run_pass is not None:
                self.run_pass(mt, *args, **options)
            return mt

        if py_func is not None:
            return wrapper(py_func)
        return wrapper


def dialect_group(dialects: Iterable[Union["Dialect", ModuleType]]):
    # NOTE: do not alias the annotation below
    def wrapper(
        transform: RunPassGen[PassParams],
    ) -> DialectGroup[PassParams]:
        ret = DialectGroup(dialects, run_pass=transform)
        return ret

    return wrapper
