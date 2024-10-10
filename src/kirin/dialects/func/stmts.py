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
        with printer.rich(style="red"):
            printer.print_str(self.name + " ")

        with printer.rich(style="cyan"):
            printer.print_str(self.sym_name)

        self.signature.print_impl(printer)
        printer.print_str(" ")
        self.body.print_impl(printer)

        with printer.rich(style="black"):
            printer.print_str(f" // func.func {self.sym_name}")


@statement(dialect=dialect, init=False)
class Call(Statement):
    name = "call"
    # not a fixed type here so just any
    callee: SSAValue = info.argument()
    kwargs: data.PyAttr[tuple[str, ...]] = info.attribute(property=True)

    def __init__(
        self,
        callee: SSAValue,
        args: Sequence[SSAValue],
        kwargs: Sequence[str] = (),
        return_types: Sequence[TypeAttribute] = (),
    ) -> None:
        super().__init__(
            args=[callee, *args],
            result_types=return_types,
            properties={"kwargs": data.PyAttr(tuple(kwargs))},
            args_slice={"callee": 0},
        )

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style="red"):
            printer.print_str("call ")

        def print_pair(pair):
            key, arg = pair
            printer.print_str(f"{key}=")
            printer.print(arg)

        n_total = len(self.args)
        callee = self.args[0]
        printer.print(callee)
        positional = self.args[1 : n_total - len(self.kwargs.data)]
        kwargs = dict(
            zip(self.kwargs.data, self.args[n_total - len(self.kwargs.data) :])
        )

        printer.print_str("(")
        printer.show_list(positional)
        if kwargs:
            printer.print_str(", ")
        printer.show_list(kwargs, print_fn=print_pair)
        printer.print_str(")")

        with printer.rich(style="black"):
            printer.print_str(" : ")
            printer.show_function_types(
                [arg.type for arg in self.args], [res.type for res in self._results]
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
        with printer.rich(style="red"):
            printer.print_str(self.name)

        if len(self.args) > 0:
            printer.print_str(" ")
            printer.show_list(self.args)


@statement(dialect=dialect, init=False)
class Lambda(Statement):
    name = "lambda"
    traits = frozenset({SymbolOpInterface(), FuncOpCallableInterface()})
    sym_name: str = info.attribute(property=True)
    signature: Signature = info.attribute()
    body: Region = info.region(multi=True)
    result: ResultValue = info.result(MethodType)

    def __init__(
        self,
        name: str,
        signature: Signature,
        captured: Sequence[SSAValue],
        code: Region,
    ):
        super().__init__(
            args=captured,
            regions=(code,),
            properties={
                "sym_name": data.PyAttr(name),
            },
            attributes={
                "signature": signature,
            },
            result_types=[MethodType],
        )

    def print_impl(self, printer: Printer) -> None:
        with printer.rich(style="red"):
            printer.print_str(self.name + " ")

        with printer.rich(style="cyan"):
            printer.print_str(self.sym_name)

        printer.print_str("(")
        printer.show_list(self.args)
        printer.print_str(")")

        with printer.rich(style="black"):
            printer.print_str(" : -> (")
            self.signature.print_impl(printer)
            printer.print_str(")")

        printer.print_str(" ")
        self.body.print_impl(printer)

        with printer.rich(style="black"):
            printer.print_str(f" // func.lambda {self.sym_name}")


@statement(dialect=dialect)
class GetField(Statement):
    name = "getfield"
    traits = frozenset({Pure()})
    obj: SSAValue = info.argument(MethodType)
    field: int = info.attribute(property=True)
    # NOTE: mypy somehow doesn't understand default init=False
    result: ResultValue = info.result(init=False)

    def print_impl(self, printer: Printer) -> None:
        printer.show_name(self)
        printer.print_str("(")
        printer.print_str(printer.ssa.get_name(self.obj))
        printer.print_str(", ")
        printer.print_str(str(self.field))
        printer.print_str(")")
        printer.print_str(" : ")
        with printer.rich(style="black"):
            printer.print(self.result.type)
