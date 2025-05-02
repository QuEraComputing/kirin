from __future__ import annotations

import typing
from types import ModuleType

# from typing import TYPE_CHECKING, Generic, TypeVar, Callable, ParamSpec
from dataclasses import field, dataclass

from kirin.print.printer import Printer
from kirin.print.printable import Printable

from .traits import (
    HasSignature,
    SymbolOpInterface,
    EntryPointInterface,
    CallableStmtInterface,
)
from .exception import ValidationError
from .nodes.stmt import Statement
from .attrs.types import Generic

if typing.TYPE_CHECKING:
    from kirin.ir.group import DialectGroup

Param = typing.ParamSpec("Param")
RetType = typing.TypeVar("RetType")


@dataclass
class Method(Printable, typing.Generic[Param, RetType]):
    dialects: "DialectGroup"  # own
    """The dialects that creates the method. This should be a DialectGroup."""
    code: Statement  # own, the corresponding IR, a func.func usually
    """The code of the method. This should be a statement with CallableStmtInterface trait."""
    nargs: int
    """The number of arguments of the method. 0 if no arguments."""
    mod: ModuleType | None = None  # ref
    """The module where the method is defined. None if no module."""
    py_func: typing.Callable[Param, RetType] | None = None  # ref
    """The original Python function. None if no Python function."""
    sym_name: str | None = None
    """The name of the method. None if no name."""
    arg_names: list[str] | None = None
    """The argument names of the callable statement. None if no keyword arguments allowed."""
    # values contained if closure
    fields: tuple = field(default_factory=tuple)  # own
    """values captured in the method if it is a closure."""
    file: str = ""
    """The file where the method is defined. Empty string if no file."""
    lineno_begin: int = 0
    """The line number where the method is defined. 0 if no line number."""
    inferred: bool = False
    """if typeinfer has been run on this method
    """

    def __init__(
        self,
        dialects: DialectGroup,
        code: Statement,
        *,
        nargs: int | None = None,
        mod: ModuleType | None = None,
        py_func: typing.Callable[Param, RetType] | None = None,
        sym_name: str | None = None,
        arg_names: list[str] | None = None,
        fields: tuple = (),
        file: str = "",
        lineno_begin: int = 0,
        inferred: bool = False,
    ):
        callable_node = code
        if entry_point := code.get_trait(EntryPointInterface):
            callable_node = entry_point.get_entry_point(code)

        trait = callable_node.get_present_trait(CallableStmtInterface)
        region = trait.get_callable_region(callable_node)
        if sym_name is None and (
            symbol_trait := callable_node.get_trait(SymbolOpInterface)
        ):
            sym_name = symbol_trait.get_sym_name(callable_node).data

        assert (
            len(region.blocks[0].args) > 0
        ), "Method must have at least have a self argument"

        self.dialects = dialects
        self.code = code
        self.nargs = nargs if nargs is not None else len(region.blocks[0].args)
        self.mod = mod
        self.py_func = py_func
        self.sym_name = sym_name
        self.arg_names = arg_names or [
            arg.name or f"arg{i}" for i, arg in enumerate(region.blocks[0].args)
        ]
        self.fields = fields
        self.file = file
        self.lineno_begin = lineno_begin
        self.inferred = inferred

    def __hash__(self) -> int:
        return id(self)

    def __call__(self, *args: Param.args, **kwargs: Param.kwargs) -> RetType:
        from kirin.interp.concrete import Interpreter

        if len(args) + len(kwargs) != self.nargs - 1:
            raise ValueError(
                f"Incorrect number of arguments, expected {self.nargs - 1}, got {len(args) + len(kwargs)}"
            )
        # NOTE: multi-return values will be wrapped in a tuple for Python
        interp = Interpreter(self.dialects)
        _, ret = interp.run(self, *args, **kwargs)
        return ret

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
        name = self.sym_name or "<lambda>"
        return f'Method("{name}")'

    def print_impl(self, printer: Printer) -> None:
        return printer.print(self.code)

    def similar(self, dialects: typing.Optional["DialectGroup"] = None):
        return Method(
            dialects=dialects or self.dialects,
            code=self.code.from_stmt(self.code, regions=[self.callable_region.clone()]),
            nargs=self.nargs,
            mod=self.mod,
            py_func=self.py_func,
            sym_name=self.sym_name,
            arg_names=self.arg_names,
            fields=self.fields,
            file=self.file,
            lineno_begin=self.lineno_begin,
            inferred=self.inferred,
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
