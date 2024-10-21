from typing import Sequence

from kirin.decl import info, statement
from kirin.dialects.func.attrs import MethodType, Signature
from kirin.dialects.func.dialect import dialect
from kirin.dialects.py import data
from kirin.ir import (
    CallableStmtInterface,
    HasParent,
    HasSignature,
    IsolatedFromAbove,
    IsTerminator,
    Pure,
    Region,
    ResultValue,
    Statement,
    SymbolOpInterface,
)
from kirin.ir.attrs import TypeAttribute
from kirin.ir.ssa import SSAValue
from kirin.print.printer import Printer


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
        }
    )
    sym_name: str = info.attribute(property=True)
    signature: Signature = info.attribute()
    body: Region = info.region(multi=True)

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style=printer.color.keyword):
            printer.print_name(self)
            printer.plain_print(" ")

        with printer.rich(style=printer.color.symbol):
            printer.plain_print(self.sym_name)

        printer.print_seq(self.signature.inputs, prefix="(", suffix=")", delim=", ")

        with printer.rich(style=printer.color.comment):
            printer.plain_print(" -> ")
            printer.print(self.signature.output)
            printer.plain_print(" ")

        printer.print(self.body)

        with printer.rich(style=printer.color.comment):
            printer.plain_print(f" // func.func {self.sym_name}")


@statement(dialect=dialect, init=False)
class Call(Statement):
    name = "call"
    # not a fixed type here so just any
    callee: SSAValue = info.argument()
    inputs: tuple[SSAValue, ...] = info.argument()
    kwargs: data.PyAttr[tuple[str, ...]] = info.attribute(property=True)
    result: ResultValue = info.result()

    def __init__(
        self,
        callee: SSAValue,
        args: Sequence[SSAValue],
        *,
        kwargs: Sequence[str] = (),
        return_type: TypeAttribute,
    ) -> None:
        super().__init__(
            args=[callee, *args],
            result_types=(return_type,),
            properties={"kwargs": data.PyAttr(tuple(kwargs))},
            args_slice={"callee": 0, "inputs": slice(1, None)},
        )

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style="red"):
            printer.print_name(self)
        printer.plain_print(" ")

        n_total = len(self.args)
        callee = self.callee
        printer.plain_print(printer.state.ssa_id[callee])

        inputs = self.inputs
        positional = inputs[: n_total - len(self.kwargs.data)]
        kwargs = dict(zip(self.kwargs.data, inputs[n_total - len(self.kwargs.data) :]))

        printer.plain_print("(")
        printer.print_seq(positional)
        if kwargs:
            printer.plain_print(", ")
        printer.print_mapping(kwargs, delim=", ")
        printer.plain_print(")")

        with printer.rich(style="black"):
            printer.plain_print(" : ")
            printer.print_seq(
                [result.type for result in self._results],
                delim=", ",
            )


@statement(dialect=dialect, init=False)
class Return(Statement):
    name = "return"
    traits = frozenset({IsTerminator(), HasParent((Function,))})

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

        super().__init__(args=args)

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style=printer.color.keyword):
            printer.print_name(self)

        if self.args:
            printer.plain_print(" ")
            printer.print_seq(self.args, delim=", ")


@statement(dialect=dialect)
class Lambda(Statement):
    name = "lambda"
    traits = frozenset({SymbolOpInterface(), FuncOpCallableInterface()})
    sym_name: str = info.attribute(property=True)
    signature: Signature = info.attribute()
    captured: tuple[SSAValue, ...] = info.argument()
    body: Region = info.region(multi=True)
    result: ResultValue = info.result(MethodType)

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style=printer.color.keyword):
            printer.print_name(self)
        printer.plain_print(" ")

        with printer.rich(style=printer.color.symbol):
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
    field: int = info.attribute(property=True)
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
