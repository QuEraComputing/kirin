from dataclasses import dataclass, field
from types import ModuleType
from typing import TYPE_CHECKING, Callable, Generic, ParamSpec, TypeVar

from kirin.exceptions import InterpreterError
from kirin.ir.attrs import TypeAttribute
from kirin.ir.nodes.stmt import Statement
from kirin.ir.traits import CallableStmtInterface
from kirin.print.printable import Printable
from kirin.print.printer import Printer

if TYPE_CHECKING:
    from kirin.ir.group import DialectGroup

Param = ParamSpec("Param")
RetType = TypeVar("RetType")


@dataclass
class Method(Printable, Generic[Param, RetType]):
    mod: ModuleType | None  # ref
    py_func: Callable[Param, RetType] | None  # ref
    sym_name: str | None
    arg_names: list[str]
    dialects: "DialectGroup"  # own
    code: Statement  # own, the corresponding IR, a func.func usually
    # values contained if closure
    fields: tuple = field(default_factory=tuple)  # own
    source: str = ""
    lineno: list[tuple[int, int]] = field(default_factory=list)
    """(<line>, <col>) at the start of the statement call.
    """
    backedges: list["Method"] = field(default_factory=list)  # own
    return_type: TypeAttribute | None = None
    inferred: bool = False
    """if typeinfer has been run on this method
    """
    verified: bool = False
    """if `code.verify` has been run on this method
    """

    def __call__(self, *args: Param.args, **kwargs: Param.kwargs) -> RetType:
        from kirin.interp.concrete import Interpreter

        if len(args) + len(kwargs) != len(self.arg_names) - 1:
            raise InterpreterError("Incorrect number of arguments")
        # NOTE: multi-return values will be wrapped in a tuple for Python
        return Interpreter(self.dialects).eval(self, args=args, kwargs=kwargs).expect()

    @property
    def args(self):
        trait = self.code.get_trait(CallableStmtInterface)
        if trait is None:
            raise ValueError("Method body must implement CallableStmtInterface")
        body = trait.get_callable_region(self.code)
        return tuple(arg for arg in body.blocks[0].args[1:])

    @property
    def arg_types(self):
        return tuple(arg.type for arg in self.args)

    def __repr__(self) -> str:
        return f'Method("{self.sym_name}")'

    def print_impl(self, printer: Printer) -> None:
        return printer.print(self.code)

    def verify(self) -> None:
        """verify the method body."""
        self.code.verify()
        self.verified = True
        return None
