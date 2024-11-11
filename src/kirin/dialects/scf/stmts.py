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
    body: ir.Region = info.region()
    result: ir.ResultValue = info.result()


@statement(dialect=dialect)
class Yield(ir.Statement):
    name = "yield"
    traits = frozenset({ir.IsTerminator()})
    values: tuple[ir.SSAValue, ...] = info.argument()
