from types import MethodType as ClassMethodType, FunctionType

from kirin import ir, types
from kirin.decl import info, statement
from kirin.print.printer import Printer
from kirin.dialects.func.attrs import Signature, MethodType
from kirin.dialects.func.dialect import dialect

from .._pprint_helper import pprint_calllike


class FuncOpCallableInterface(ir.CallableStmtInterface["Function"]):

    @classmethod
    def get_callable_region(cls, stmt: "Function") -> ir.Region:
        return stmt.body


@statement(dialect=dialect)
class Function(ir.Statement):
    name = "func"
    traits = frozenset(
        {
            ir.IsolatedFromAbove(),
            ir.SymbolOpInterface(),
            ir.HasSignature(),
            FuncOpCallableInterface(),
            ir.SSACFGRegion(),
        }
    )
    sym_name: str = info.attribute()
    """The symbol name of the function."""
    signature: Signature = info.attribute()
    body: ir.Region = info.region(multi=True)

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
class Call(ir.Statement):
    name = "call"
    traits = frozenset({ir.MaybePure()})
    # not a fixed type here so just any
    callee: ir.SSAValue = info.argument()
    inputs: tuple[ir.SSAValue, ...] = info.argument()
    kwargs: tuple[str, ...] = info.attribute(default_factory=lambda: ())
    result: ir.ResultValue = info.result()
    purity: bool = info.attribute(default=False)

    def print_impl(self, printer: Printer) -> None:
        pprint_calllike(self, printer.state.ssa_id[self.callee], printer)

    def check_type(self) -> None:
        if not self.callee.type.is_subseteq(types.MethodType):
            if self.callee.type.is_subseteq(types.PyClass(FunctionType)):
                raise ir.TypeCheckError(
                    self,
                    f"callee must be a method type, got {self.callee.type}",
                    help="did you call a Python function directly? "
                    "consider decorating it with kernel decorator",
                )

            if self.callee.type.is_subseteq(types.PyClass(ClassMethodType)):
                raise ir.TypeCheckError(
                    self,
                    "callee must be a method type, got class method",
                    help="did you try to call a Python class method within a kernel? "
                    "consider rewriting it with a captured variable instead of calling it inside the kernel",
                )

            if self.callee.type is types.Any:
                return
            raise ir.TypeCheckError(
                self,
                f"callee must be a method type, got {self.callee.type}",
                help="did you forget to decorate the function with kernel decorator?",
            )


@statement(dialect=dialect)
class ConstantNone(ir.Statement):
    """A constant None value.

    This is mainly used to represent the None return value of a function
    to match Python semantics.
    """

    name = "const.none"
    traits = frozenset({ir.Pure(), ir.ConstantLike()})
    result: ir.ResultValue = info.result(types.NoneType)


@statement(dialect=dialect, init=False)
class Return(ir.Statement):
    name = "return"
    traits = frozenset({ir.IsTerminator(), ir.HasParent((Function,))})
    value: ir.SSAValue = info.argument()

    def __init__(self, value_or_stmt: ir.SSAValue | ir.Statement | None = None) -> None:
        if isinstance(value_or_stmt, ir.SSAValue):
            args = [value_or_stmt]
        elif isinstance(value_or_stmt, ir.Statement):
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

    def check(self) -> None:
        assert self.args, "return statement must have at least one value"
        assert len(self.args) <= 1, (
            "return statement must have at most one value"
            ", wrap multiple values in a tuple"
        )


@statement(dialect=dialect)
class Lambda(ir.Statement):
    name = "lambda"
    traits = frozenset(
        {
            ir.Pure(),
            ir.HasSignature(),
            ir.SymbolOpInterface(),
            FuncOpCallableInterface(),
            ir.SSACFGRegion(),
        }
    )
    sym_name: str = info.attribute()
    signature: Signature = info.attribute()
    captured: tuple[ir.SSAValue, ...] = info.argument()
    body: ir.Region = info.region(multi=True)
    result: ir.ResultValue = info.result(MethodType)

    def check(self) -> None:
        assert self.body.blocks, "lambda body must not be empty"

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
class GetField(ir.Statement):
    name = "getfield"
    traits = frozenset({ir.Pure()})
    obj: ir.SSAValue = info.argument(MethodType)
    field: int = info.attribute()
    # NOTE: mypy somehow doesn't understand default init=False
    result: ir.ResultValue = info.result(init=False)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(
            "(", printer.state.ssa_id[self.obj], ", ", str(self.field), ")"
        )
        with printer.rich(style="black"):
            printer.plain_print(" : ")
            printer.print(self.result.type)


@statement(dialect=dialect)
class Invoke(ir.Statement):
    name = "invoke"
    traits = frozenset({ir.MaybePure()})
    callee: ir.Method = info.attribute()
    inputs: tuple[ir.SSAValue, ...] = info.argument()
    kwargs: tuple[str, ...] = info.attribute()
    result: ir.ResultValue = info.result()
    purity: bool = info.attribute(default=False)

    def print_impl(self, printer: Printer) -> None:
        pprint_calllike(self, self.callee.sym_name, printer)

    def check(self) -> None:
        if self.kwargs:
            for name in self.kwargs:
                assert (
                    name in self.callee.arg_names
                ), f"method {self.callee.sym_name} does not have argument {name}"
        elif len(self.callee.arg_names) - 1 != len(self.args):
            raise ValueError(
                f"expected {len(self.callee.arg_names)} arguments, got {len(self.args)}"
            )
