import typing
from types import ModuleType

# from typing import TYPE_CHECKING, Generic, TypeVar, Callable, ParamSpec
from dataclasses import field, dataclass

from kirin.ir.traits import HasSignature, CallableStmtInterface
from kirin.ir.exception import ValidationError
from kirin.ir.nodes.stmt import Statement
from kirin.print.printer import Printer
from kirin.ir.attrs.types import Generic
from kirin.print.printable import Printable

if typing.TYPE_CHECKING:
    from kirin.ir.group import DialectGroup

Param = typing.ParamSpec("Param")
RetType = typing.TypeVar("RetType")


@dataclass
class Method(Printable, typing.Generic[Param, RetType]):
    mod: ModuleType | None  # ref
    py_func: typing.Callable[Param, RetType] | None  # ref
    sym_name: str
    arg_names: list[str]
    dialects: "DialectGroup"  # own
    code: Statement  # own, the corresponding IR, a func.func usually
    # values contained if closure
    fields: tuple = field(default_factory=tuple)  # own
    file: str = ""
    lineno_begin: int = 0
    inferred: bool = False
    """if typeinfer has been run on this method
    """

    def __hash__(self) -> int:
        return id(self)

    def __call__(self, *args: Param.args, **kwargs: Param.kwargs) -> RetType:
        from kirin.interp.concrete import Interpreter

        if len(args) + len(kwargs) != len(self.arg_names) - 1:
            raise ValueError("Incorrect number of arguments")
        # NOTE: multi-return values will be wrapped in a tuple for Python
        interp = Interpreter(self.dialects)
        return interp.run(self, args=args, kwargs=kwargs)

    @property
    def args(self):
        """Return the arguments of the method. (excluding self)"""
        return tuple(arg for arg in self.callable_region.blocks[0].args[1:])

    @property
    def arg_types(self):
        """Return the types of the arguments of the method. (excluding self)"""
        return tuple(arg.type for arg in self.args)

    @property
    def self_type(self):
        """Return the type of the self argument of the method."""
        trait = self.code.get_present_trait(HasSignature)
        signature = trait.get_signature(self.code)
        return Generic(Method, Generic(tuple, *signature.inputs), signature.output)

    @property
    def callable_region(self):
        trait = self.code.get_present_trait(CallableStmtInterface)
        return trait.get_callable_region(self.code)

    @property
    def return_type(self):
        trait = self.code.get_present_trait(HasSignature)
        return trait.get_signature(self.code).output

    def __repr__(self) -> str:
        return f'Method("{self.sym_name}")'

    def print_impl(self, printer: Printer) -> None:
        return printer.print(self.code)

    def similar(self, dialects: typing.Optional["DialectGroup"] = None):
        return Method(
            self.mod,
            self.py_func,
            self.sym_name,
            self.arg_names,
            dialects or self.dialects,
            self.code.from_stmt(self.code, regions=[self.callable_region.clone()]),
            self.fields,
            self.file,
            self.inferred,
        )

    def verify(self) -> None:
        """verify the method body.

        This will raise a ValidationError if the method body is not valid.
        """
        try:
            self.code.verify()
        except ValidationError as e:
            e.attach(self)
            raise e

    def verify_type(self) -> None:
        """verify the method type.

        This will raise a ValidationError if the method type is not valid.
        """
        # NOTE: verify the method body
        self.verify()

        try:
            self.code.verify_type()
        except ValidationError as e:
            e.attach(self)
            raise e
