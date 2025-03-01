import inspect
from types import ModuleType
from typing import (
    TYPE_CHECKING,
    Union,
    Generic,
    TypeVar,
    Callable,
    ParamSpec,
    Concatenate,
    overload,
)
from functools import update_wrapper
from dataclasses import dataclass
from collections.abc import Iterable

from kirin.ir.method import Method
from kirin.exceptions import CompilerError

if TYPE_CHECKING:
    from kirin.registry import Registry
    from kirin.ir.dialect import Dialect

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
    """The set of dialects in the group."""
    # NOTE: this is used to create new dialect groups from existing one
    run_pass_gen: RunPassGen[PassParams] | None = None
    """the function that generates the `run_pass` function.

    This is used to create new dialect groups from existing ones, while
    keeping the same `run_pass` function.
    """
    run_pass: RunPass[PassParams] | None = None
    """the function that runs the passes on the method."""

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

    def __repr__(self) -> str:
        names = ", ".join(each.name for each in self.data)
        return f"DialectGroup([{names}])"

    @staticmethod
    def map_module(dialect: Union["Dialect", ModuleType]) -> "Dialect":
        """map the module to the dialect if it is a module.
        It assumes that the module has a `dialect` attribute
        that is an instance of [`Dialect`][kirin.ir.Dialect].
        """
        if isinstance(dialect, ModuleType):
            return getattr(dialect, "dialect")
        return dialect

    def add(self, dialect: Union["Dialect", ModuleType]) -> "DialectGroup":
        """add a dialect to the group.

        Args:
            dialect (Union[Dialect, ModuleType]): the dialect to add

        Returns:
            DialectGroup: the new dialect group with the added
        """
        return self.union([dialect])

    def union(self, dialect: Iterable[Union["Dialect", ModuleType]]) -> "DialectGroup":
        """union a set of dialects to the group.

        Args:
            dialect (Iterable[Union[Dialect, ModuleType]]): the dialects to union

        Returns:
            DialectGroup: the new dialect group with the union.
        """
        return DialectGroup(
            dialects=self.data.union(frozenset(self.map_module(d) for d in dialect)),
            run_pass=self.run_pass_gen,  # pass the run_pass_gen function
        )

    def discard(self, dialect: Union["Dialect", ModuleType]) -> "DialectGroup":
        """discard a dialect from the group.

        !!! note
            This does not raise an error if the dialect is not in the group.

        Args:
            dialect (Union[Dialect, ModuleType]): the dialect to discard

        Returns:
            DialectGroup: the new dialect group with the discarded dialect.
        """
        dialect_ = self.map_module(dialect)
        return DialectGroup(
            dialects=frozenset(
                each for each in self.data if each.name != dialect_.name
            ),
            run_pass=self.run_pass_gen,  # pass the run_pass_gen function
        )

    def __contains__(self, dialect) -> bool:
        """check if the dialect is in the group.

        Args:
            dialect (Union[Dialect, ModuleType]): the dialect to check.

        Returns:
            bool: True if the dialect is in the group, False otherwise.
        """
        return self.map_module(dialect) in self.data

    @property
    def registry(self) -> "Registry":
        """return the registry for the dialect group. This
        returns a proxy object that can be used to select
        the lowering interpreters, interpreters, and codegen
        for the dialects in the group.

        Returns:
            Registry: the registry object.
        """
        from kirin.registry import Registry

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
        """create a method from the python function.

        Args:
            py_func (Callable): the python function to create the method from.
            args (PassParams.args): the arguments to pass to the run_pass function.
            options (PassParams.kwargs): the keyword arguments to pass to the run_pass function.

        Returns:
            Method: the method created from the python function.
        """
        from kirin.lowering import Lowering

        emit_ir = Lowering(self)

        def wrapper(py_func: Callable) -> Method:
            if py_func.__name__ == "<lambda>":
                raise ValueError("Cannot compile lambda functions")

            lineno_offset, file = 0, ""
            frame = inspect.currentframe()
            if frame and frame.f_back is not None and frame.f_back.f_back is not None:
                call_site_frame = frame.f_back.f_back
                if py_func.__name__ in call_site_frame.f_locals:
                    raise CompilerError(
                        f"overwriting function definition of `{py_func.__name__}`"
                    )

                lineno_offset = call_site_frame.f_lineno - 1
                file = call_site_frame.f_code.co_filename

            code = emit_ir.run(py_func, lineno_offset=lineno_offset)
            mt = Method(
                mod=inspect.getmodule(py_func),
                py_func=py_func,
                sym_name=py_func.__name__,
                arg_names=["#self#"] + inspect.getfullargspec(py_func).args,
                dialects=self,
                code=code,
                file=file,
            )
            if doc := inspect.getdoc(py_func):
                mt.__doc__ = doc

            if self.run_pass is not None:
                self.run_pass(mt, *args, **options)
            return mt

        if py_func is not None:
            return wrapper(py_func)
        return wrapper


def dialect_group(
    dialects: Iterable[Union["Dialect", ModuleType]],
) -> Callable[[RunPassGen[PassParams]], DialectGroup[PassParams]]:
    """Create a dialect group from the given dialects based on the
    definition of `run_pass` function.

    Args:
        dialects (Iterable[Union[Dialect, ModuleType]]): the dialects to include in the group.

    Returns:
        Callable[[RunPassGen[PassParams]], DialectGroup[PassParams]]: the dialect group.

    Example:
        ```python
        from kirin.dialects import cf, fcf, func, math

        @dialect_group([cf, fcf, func, math])
        def basic_no_opt(self):
            # initializations
            def run_pass(mt: Method) -> None:
                # how passes are applied to the method
                pass

            return run_pass
        ```
    """

    # NOTE: do not alias the annotation below
    def wrapper(
        transform: RunPassGen[PassParams],
    ) -> DialectGroup[PassParams]:
        ret = DialectGroup(dialects, run_pass=transform)
        update_wrapper(ret, transform)
        return ret

    return wrapper
