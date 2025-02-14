from kirin import types
from kirin.ir import (
    Pure,
    Method,
    Region,
    HasParent,
    MaybePure,
    Statement,
    ResultValue,
    ConstantLike,
    HasSignature,
    IsTerminator,
    SSACFGRegion,
    IsolatedFromAbove,
    SymbolOpInterface,
    CallableStmtInterface,
)
from kirin.decl import info, statement
from kirin.ir.ssa import SSAValue
from kirin.exceptions import VerificationError
from kirin.print.printer import Printer
from kirin.dialects.func.attrs import Signature, MethodType
from kirin.dialects.func.dialect import dialect

from .._pprint_helper import pprint_calllike


class FuncOpCallableInterface(CallableStmtInterface["Function"]):

    @classmethod
    def get_callable_region(cls, stmt: "Function") -> Region:
        return stmt.body


@statement(dialect=dialect)
class Function(Statement):
    name = "func"
    traits = frozenset(
        {
            IsolatedFromAbove(),
            SymbolOpInterface(),
            HasSignature(),
            FuncOpCallableInterface(),
            SSACFGRegion(),
        }
    )
    sym_name: str = info.attribute()
    """The symbol name of the function."""
    signature: Signature = info.attribute()
    body: Region = info.region(multi=True)

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style="keyword"):
            printer.print_name(self)
            printer.plain_print(" ")

        with printer.rich(style="symbol"):
            printer.plain_print(self.sym_name)

        printer.print_seq(self.signature.inputs, prefix="(", suffix=")", delim=", ")

        with printer.rich(style="comment"):
            printer.plain_print(" -> ")
            printer.print(self.signature.output)
            printer.plain_print(" ")

        printer.print(self.body)

        with printer.rich(style="comment"):
            printer.plain_print(f" // func.func {self.sym_name}")


@statement(dialect=dialect)
class Call(Statement):
    name = "call"
    traits = frozenset({MaybePure()})
    # not a fixed type here so just any
    callee: SSAValue = info.argument()
    inputs: tuple[SSAValue, ...] = info.argument()
    kwargs: tuple[str, ...] = info.attribute(default_factory=lambda: ())
    result: ResultValue = info.result()
    purity: bool = info.attribute(default=False)

    def print_impl(self, printer: Printer) -> None:
        pprint_calllike(self, printer.state.ssa_id[self.callee], printer)


@statement(dialect=dialect)
class ConstantNone(Statement):
    """A constant None value.

    This is mainly used to represent the None return value of a function
    to match Python semantics.
    """

    name = "const.none"
    traits = frozenset({Pure(), ConstantLike()})
    result: ResultValue = info.result(types.NoneType)


@statement(dialect=dialect, init=False)
class Return(Statement):
    name = "return"
    traits = frozenset({IsTerminator(), HasParent((Function,))})
    value: SSAValue = info.argument()

    def __init__(self, value_or_stmt: SSAValue | Statement | None = None) -> None:
        if isinstance(value_or_stmt, SSAValue):
            args = [value_or_stmt]
        elif isinstance(value_or_stmt, Statement):
            if len(value_or_stmt._results) == 1:
                args = [value_or_stmt._results[0]]
            else:
                raise ValueError(
                    f"expected a single result, got {len(value_or_stmt._results)} results from {value_or_stmt.name}"
                )
        elif value_or_stmt is None:
            args = []
        else:
            raise ValueError(f"expected SSAValue or Statement, got {value_or_stmt}")

        super().__init__(args=args, args_slice={"value": 0})

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style="keyword"):
            printer.print_name(self)

        if self.args:
            printer.plain_print(" ")
            printer.print_seq(self.args, delim=", ")

    def verify(self) -> None:
        if not self.args:
            raise VerificationError(
                self, "return statement must have at least one value"
            )

        if len(self.args) > 1:
            raise VerificationError(
                self,
                "return statement must have at most one value"
                ", wrap multiple values in a tuple",
            )


@statement(dialect=dialect)
class Lambda(Statement):
    name = "lambda"
    traits = frozenset(
        {
            Pure(),
            HasSignature(),
            SymbolOpInterface(),
            FuncOpCallableInterface(),
            SSACFGRegion(),
        }
    )
    sym_name: str = info.attribute()
    signature: Signature = info.attribute()
    captured: tuple[SSAValue, ...] = info.argument()
    body: Region = info.region(multi=True)
    result: ResultValue = info.result(MethodType)

    def verify(self) -> None:
        if self.body.blocks.isempty():
            raise VerificationError(self, "lambda body must not be empty")

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style="keyword"):
            printer.print_name(self)
        printer.plain_print(" ")

        with printer.rich(style="symbol"):
            printer.plain_print(self.sym_name)

        printer.print_seq(self.captured, prefix="(", suffix=")", delim=", ")

        with printer.rich(style="bright_black"):
            printer.plain_print(" -> ")
            printer.print(self.signature.output)

        printer.plain_print(" ")
        printer.print(self.body)

        with printer.rich(style="black"):
            printer.plain_print(f" // func.lambda {self.sym_name}")


@statement(dialect=dialect)
class GetField(Statement):
    name = "getfield"
    traits = frozenset({Pure()})
    obj: SSAValue = info.argument(MethodType)
    field: int = info.attribute()
    # NOTE: mypy somehow doesn't understand default init=False
    result: ResultValue = info.result(init=False)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(
            "(", printer.state.ssa_id[self.obj], ", ", str(self.field), ")"
        )
        with printer.rich(style="black"):
            printer.plain_print(" : ")
            printer.print(self.result.type)


@statement(dialect=dialect)
class Invoke(Statement):
    name = "invoke"
    traits = frozenset({MaybePure()})
    callee: Method = info.attribute()
    inputs: tuple[SSAValue, ...] = info.argument()
    kwargs: tuple[str, ...] = info.attribute()
    result: ResultValue = info.result()
    purity: bool = info.attribute(default=False)

    def print_impl(self, printer: Printer) -> None:
        pprint_calllike(self, self.callee.sym_name, printer)

    def verify(self) -> None:
        if self.kwargs:
            for name in self.kwargs:
                if name not in self.callee.arg_names:
                    raise VerificationError(
                        self,
                        f"method {self.callee.sym_name} does not have argument {name}",
                    )
        elif len(self.callee.arg_names) - 1 != len(self.args):
            raise VerificationError(
                self,
                f"expected {len(self.callee.arg_names)} arguments, got {len(self.args)}",
            )
