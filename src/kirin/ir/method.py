from types import ModuleType
from typing import TYPE_CHECKING, Generic, TypeVar, Callable, ParamSpec
from dataclasses import field, dataclass

from kirin.ir.traits import HasSignature, CallableStmtInterface
from kirin.exceptions import InterpreterError, VerificationError
from kirin.ir.nodes.stmt import Statement
from kirin.print.printer import Printer
from kirin.print.printable import Printable

if TYPE_CHECKING:
    from kirin.ir.group import DialectGroup

Param = ParamSpec("Param")
RetType = TypeVar("RetType")


@dataclass
class Method(Printable, Generic[Param, RetType]):
    mod: ModuleType | None  # ref
    py_func: Callable[Param, RetType] | None  # ref
    sym_name: str
    arg_names: list[str]
    dialects: "DialectGroup"  # own
    code: Statement  # own, the corresponding IR, a func.func usually
    # values contained if closure
    fields: tuple = field(default_factory=tuple)  # own
    file: str = ""
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
        return tuple(arg for arg in self.callable_region.blocks[0].args[1:])

    @property
    def arg_types(self):
        return tuple(arg.type for arg in self.args)

    @property
    def callable_region(self):
        trait = self.code.get_trait(CallableStmtInterface)
        if trait is None:
            raise ValueError("Method body must implement CallableStmtInterface")
        return trait.get_callable_region(self.code)

    @property
    def return_type(self):
        trait = self.code.get_trait(HasSignature)
        if trait is None:
            raise ValueError("Method body must implement HasSignature")
        return trait.get_signature(self.code).output

    def __repr__(self) -> str:
        return f'Method("{self.sym_name}")'

    def print_impl(self, printer: Printer) -> None:
        return printer.print(self.code)

    def verify(self) -> None:
        """verify the method body."""
        try:
            self.code.verify()
        except VerificationError as e:
            msg = f'File "{self.file}"'
            if isinstance(e.node, Statement):
                if e.node.source:
                    msg += f", line {e.node.source.lineno}"
                msg += f", in {e.node.name}"

            msg += f":\n    Verification failed for {self.sym_name}: {e.args[0]}"
            raise Exception(msg) from e
        self.verified = True
        return
