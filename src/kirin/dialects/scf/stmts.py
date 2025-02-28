from kirin import ir, types
from kirin.decl import info, statement
from kirin.exceptions import VerificationError
from kirin.print.printer import Printer

from ._dialect import dialect


@statement(dialect=dialect, init=False)
class IfElse(ir.Statement):
    """Python-like if-else statement.

    This statement has a condition, then body, and else body.

    Then body either terminates with a yield statement or `scf.return`.
    """

    name = "if"
    traits = frozenset({ir.MaybePure(), ir.SSACFGRegion()})
    purity: bool = info.attribute(default=False)
    cond: ir.SSAValue = info.argument(types.Any)
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
            then_body_region = then_body
            if then_body_region.blocks:
                then_body_block = then_body_region.blocks[-1]
            else:
                then_body_block = None
        elif isinstance(then_body, ir.Block):
            then_body_block = then_body
            then_body_region = ir.Region(then_body)

        if isinstance(else_body, ir.Region):
            if not else_body.blocks:  # empty region
                else_body_region = else_body
                else_body_block = None
            elif len(else_body.blocks) == 0:
                else_body_region = else_body
                else_body_block = None
            else:
                else_body_region = else_body
                else_body_block = else_body_region.blocks[0]
        elif isinstance(else_body, ir.Block):
            else_body_region = ir.Region(else_body)
            else_body_block = else_body
        else:
            else_body_region = ir.Region()
            else_body_block = None

        # if either then or else body has yield, we generate results
        # we assume if both have yields, they have the same number of results
        results = ()
        if then_body_block is not None:
            then_yield = then_body_block.last_stmt
            else_yield = (
                else_body_block.last_stmt if else_body_block is not None else None
            )
            if then_yield is not None and isinstance(then_yield, Yield):
                results = then_yield.values
            elif else_yield is not None and isinstance(else_yield, Yield):
                results = else_yield.values

        result_types = tuple(value.type for value in results)
        super().__init__(
            args=(cond,),
            regions=(then_body_region, else_body_region),
            result_types=result_types,
            args_slice={"cond": 0},
            attributes={"purity": ir.PyAttr(False)},
        )

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        printer.print(self.cond)
        printer.plain_print(" ")
        printer.print(self.then_body)
        if self.else_body.blocks and not (
            len(self.else_body.blocks[0].stmts) == 1
            and isinstance(else_term := self.else_body.blocks[0].last_stmt, Yield)
            and not else_term.values  # empty yield
        ):
            printer.plain_print(" else ", style="keyword")
            printer.print(self.else_body)

        with printer.rich(style="comment"):
            printer.plain_print(f" -> purity={self.purity}")

    def verify(self) -> None:
        from kirin.dialects.func import Return

        if len(self.then_body.blocks) != 1:
            raise VerificationError(self, "then region must have a single block")

        if len(self.else_body.blocks) != 1:
            raise VerificationError(self, "else region must have a single block")

        then_block = self.then_body.blocks[0]
        else_block = self.else_body.blocks[0]
        if len(then_block.args) != 1:
            raise VerificationError(
                self, "then block must have a single argument for condition"
            )

        if len(else_block.args) != 1:
            raise VerificationError(
                self, "else block must have a single argument for condition"
            )

        then_stmt = then_block.last_stmt
        else_stmt = else_block.last_stmt
        if then_stmt is None or not isinstance(then_stmt, (Yield, Return)):
            raise VerificationError(
                self, "then block must terminate with a yield or return"
            )

        if else_stmt is None or not isinstance(else_stmt, (Yield, Return)):
            raise VerificationError(
                self, "else block must terminate with a yield or return"
            )


@statement(dialect=dialect, init=False)
class For(ir.Statement):
    name = "for"
    traits = frozenset({ir.MaybePure(), ir.SSACFGRegion()})
    purity: bool = info.attribute(default=False)
    iterable: ir.SSAValue = info.argument(types.Any)
    body: ir.Region = info.region(multi=False)
    initializers: tuple[ir.SSAValue, ...] = info.argument(types.Any)

    def __init__(
        self,
        iterable: ir.SSAValue,
        body: ir.Region,
        *initializers: ir.SSAValue,
    ):
        stmt = body.blocks[0].last_stmt
        if isinstance(stmt, Yield):
            result_types = tuple(value.type for value in stmt.values)
        else:
            result_types = ()
        super().__init__(
            args=(iterable, *initializers),
            regions=(body,),
            result_types=result_types,
            args_slice={"iterable": 0, "initializers": slice(1, None)},
            attributes={"purity": ir.PyAttr(False)},
        )

    def verify(self) -> None:
        from kirin.dialects.func import Return

        if len(self.body.blocks) != 1:
            raise VerificationError(self, "for loop body must have a single block")

        if len(self.body.blocks[0].args) != len(self.initializers) + 1:
            raise VerificationError(
                self,
                "for loop body must have arguments for all initializers and the loop variable",
            )

        stmt = self.body.blocks[0].last_stmt
        if stmt is None or not isinstance(stmt, (Yield, Return)):
            raise VerificationError(
                self, "for loop body must terminate with a yield or return"
            )

        if isinstance(stmt, Return):
            return

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
        if self.results:
            with printer.rich(style="comment"):
                printer.plain_print(" -> ")
                printer.print_seq(
                    tuple(result.type for result in self.results),
                    delim=", ",
                    style="comment",
                )

        with printer.indent():
            if self.initializers:
                printer.print_newline()
                printer.plain_print("iter_args(")
                for idx, (arg, val) in enumerate(
                    zip(block.args[1:], self.initializers)
                ):
                    printer.print(arg)
                    printer.plain_print(" = ")
                    printer.print(val)
                    if idx < len(self.initializers) - 1:
                        printer.plain_print(", ")
                printer.plain_print(")")

            printer.plain_print(" {")
            if printer.analysis is not None:
                with printer.rich(style="warning"):
                    for arg in block.args:
                        printer.print_newline()
                        printer.print_analysis(
                            arg, prefix=f"{printer.state.ssa_id[arg]} --> "
                        )
            with printer.align(printer.result_width(block.stmts)):
                for stmt in block.stmts:
                    printer.print_newline()
                    printer.print_stmt(stmt)
        printer.print_newline()
        printer.plain_print("}")
        with printer.rich(style="comment"):
            printer.plain_print(f" -> purity={self.purity}")


@statement(dialect=dialect)
class Yield(ir.Statement):
    name = "yield"
    traits = frozenset({ir.IsTerminator()})
    values: tuple[ir.SSAValue, ...] = info.argument(types.Any)

    def __init__(self, *values: ir.SSAValue):
        super().__init__(args=values, args_slice={"values": slice(None)})

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.print_seq(self.values, prefix=" ", delim=", ")
