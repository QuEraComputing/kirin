from kirin import ir
from kirin.decl import info, statement
from kirin.exceptions import VerificationError, DialectLoweringError
from kirin.print.printer import Printer

from ._dialect import dialect


@statement(dialect=dialect, init=False)
class IfElse(ir.Statement):
    """Python-like if-else statement.

    This statement has a condition, then body, and else body.

    Then body either terminates with a yield statement or `scf.return`.
    """

    name = "if"
    cond: ir.SSAValue = info.argument(ir.types.Any)
    # NOTE: we don't enforce the type here
    # because anything implements __bool__ in Python
    # can be used as a condition
    then_body: ir.Region = info.region(multi=False)
    else_body: ir.Region = info.region(multi=False, default_factory=ir.Region)

    def __init__(
        self,
        cond: ir.SSAValue,
        then_body: ir.Region | ir.Block,
        else_body: ir.Region | ir.Block | None = None,
    ):
        if isinstance(then_body, ir.Region):
            if len(then_body.blocks) != 1:
                raise DialectLoweringError(
                    "if-else statement must have a single block in the then region"
                )
            then_body_region = then_body
            then_body = then_body_region.blocks[0]
        elif isinstance(then_body, ir.Block):
            then_body_region = ir.Region(then_body)

        if isinstance(else_body, ir.Region):
            if not else_body.blocks:  # empty region
                else_body_region = else_body
                else_body = None
            elif len(else_body.blocks) != 1:
                raise DialectLoweringError(
                    "if-else statement must have a single block in the else region"
                )
            else:
                else_body_region = else_body
                else_body = else_body_region.blocks[0]
        elif isinstance(else_body, ir.Block):
            else_body_region = ir.Region(else_body)
        else:
            else_body_region = ir.Region()

        # if then_body terminates with a yield, we have results
        then_yield = then_body.last_stmt
        if then_yield is not None and isinstance(then_yield, Yield):
            if else_body is None:
                raise DialectLoweringError(
                    "if-else statement with results must have an else block"
                )
            else_yield = else_body.last_stmt
            if else_yield is None or not isinstance(else_yield, Yield):
                raise DialectLoweringError(
                    "if-else statement with results must have a yield in the else block"
                )

            if len(else_yield.values) != len(then_yield.values):
                raise DialectLoweringError(
                    "if-else statement with results must have the same number of results in both branches"
                )

            result_types = tuple(
                then_v.type.join(else_v.type)
                for then_v, else_v in zip(then_yield.values, else_yield.values)
            )
        else:
            result_types = ()

        super().__init__(
            args=(cond,),
            regions=(then_body_region, else_body_region),
            result_types=result_types,
            args_slice={"cond": 0},
        )

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        printer.print(self.cond)
        printer.plain_print(" ")
        printer.print(self.then_body)
        if self.else_body.blocks:
            printer.plain_print(" else ", style="keyword")
            printer.print(self.else_body)


@statement(dialect=dialect, init=False)
class For(ir.Statement):
    name = "for"
    iterable: ir.SSAValue = info.argument(ir.types.Any)
    body: ir.Region = info.region(multi=False)
    initializers: tuple[ir.SSAValue, ...] = info.argument(ir.types.Any)

    def __init__(
        self,
        iterable: ir.SSAValue,
        body: ir.Region,
        *initializers: ir.SSAValue,
    ):
        stmt = body.blocks[0].last_stmt
        if stmt is None or not isinstance(stmt, Yield):
            raise DialectLoweringError("for loop body must terminate with a yield")

        if len(body.blocks) != 1:
            raise DialectLoweringError("for loop body must have a single block")

        if len(body.blocks[0].args) != len(initializers) + 1:
            raise DialectLoweringError(
                "for loop body must have arguments for all initializers and the loop variable"
            )

        super().__init__(
            args=(iterable, *initializers),
            regions=(body,),
            result_types=tuple(value.type for value in stmt.values),
            args_slice={"iterable": 0, "initializers": slice(1, None)},
        )

    def verify(self) -> None:
        stmt = self.body.blocks[0].last_stmt
        if stmt is None or not isinstance(stmt, Yield):
            raise VerificationError(self, "for loop body must terminate with a yield")
        if len(stmt.values) != len(self.initializers):
            raise VerificationError(
                self,
                "for loop body must have the same number of results as initializers",
            )
        if len(self.results) != len(stmt.values):
            raise VerificationError(
                self,
                "for loop must have the same number of results as the yield in the body",
            )

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        block = self.body.blocks[0]
        printer.print(block.args[0])
        printer.plain_print(" in ", style="keyword")
        printer.print(self.iterable)
        with printer.indent():
            printer.print_newline()
            printer.plain_print("iter_args(")
            for idx, (arg, val) in enumerate(zip(block.args[1:], self.initializers)):
                printer.print(arg)
                printer.plain_print(" = ")
                printer.print(val)
                if idx < len(self.initializers) - 1:
                    printer.plain_print(", ")
            printer.plain_print(") {")

            with printer.align(printer.result_width(block.stmts)):
                for stmt in block.stmts:
                    printer.print_newline()
                    printer.print_stmt(stmt)
        printer.print_newline()
        printer.plain_print("}")


@statement(dialect=dialect)
class Yield(ir.Statement):
    name = "yield"
    traits = frozenset({ir.IsTerminator()})
    values: tuple[ir.SSAValue, ...] = info.argument(ir.types.Any)

    def __init__(self, *values: ir.SSAValue):
        super().__init__(args=values, args_slice={"values": slice(None)})

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.print_seq(self.values, prefix=" ", delim=", ")
