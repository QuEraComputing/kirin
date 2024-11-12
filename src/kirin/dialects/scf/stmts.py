from kirin import info, ir, statement
from kirin.dialects.scf.dialect import dialect


@statement(dialect=dialect)
class Iter(ir.Statement):
    name = "iter"
    value: ir.SSAValue = info.argument()
    result: ir.ResultValue = info.result()


# NOTE: as an example
# for i in range(5):
#    print(i)
#
# is equivalent to:
# %range = py.stmts.Constant(range(5))
# %range_iter = scf.iter(%range)
# scf.for %range_iter {
#    ^bb(%i: i64): # loop variables are entry block arguments
#      print(%i)
# }
@statement(dialect=dialect)
class For(ir.Statement):
    name = "for"
    iter: ir.SSAValue = info.argument()
    body: ir.Region = info.region(multi=False)

    def __init__(self, iter: ir.SSAValue, body: ir.Region):
        assert len(body.blocks) == 1, "For loop body must have exactly one block"
        last_stmt = body.blocks[0].last_stmt
        assert isinstance(
            last_stmt, Yield
        ), "For loop body must end with a yield statement"
        result_types = tuple(v.type for v in last_stmt.values)
        super().__init__(
            args=(iter,),
            result_types=result_types,
            regions=(body,),
            args_slice={"iter": 0},
        )


@statement(dialect=dialect)
class IfElse(ir.Statement):
    name = "if"
    cond: ir.SSAValue = info.argument()
    then_body: ir.Region = info.region(multi=False)
    else_body: ir.Region = info.region(multi=False)

    def __init__(
        self,
        cond: ir.SSAValue,
        then_body: ir.Region,
        else_body: ir.Region | None = None,
    ):
        assert len(then_body.blocks) == 1, "Then block must have exactly one block"
        last_then_stmt = then_body.blocks[0].last_stmt

        result_types = ()
        if isinstance(last_then_stmt, Yield):
            result_types = tuple(v.type for v in last_then_stmt.values)

        if else_body is not None and len(else_body.blocks) > 0:
            assert len(else_body.blocks) == 1, "Else block must have exactly one block"
            last_else_stmt = else_body.blocks[0].last_stmt
            if isinstance(last_else_stmt, Yield):
                if isinstance(last_then_stmt, Yield):
                    result_types = tuple(
                        then_v.type.join(else_v.type)
                        for then_v, else_v in zip(
                            last_then_stmt.values, last_else_stmt.values
                        )
                    )
                else:
                    result_types = tuple(v.type for v in last_else_stmt.values)

        super().__init__(
            args=(cond,),
            result_types=result_types,
            regions=(then_body, else_body or ir.Region()),
            args_slice={"cond": 0},
        )


@statement(dialect=dialect)
class Yield(ir.Statement):
    name = "yield"
    traits = frozenset({ir.IsTerminator()})
    values: tuple[ir.SSAValue, ...] = info.argument()
