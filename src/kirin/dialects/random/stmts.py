from kirin import ir, types, lowering
from kirin.decl import info, statement

from ._dialect import dialect


@statement(dialect=dialect)
class Random(ir.Statement):
    """random statement, wrapping the random.random function
    returns a random floating number between 0 and 1
    """

    traits = frozenset({lowering.FromPythonCall()})
    result: ir.ResultValue = info.result(types.Float)


@statement(dialect=dialect)
class RandInt(ir.Statement):
    """randint statement, wrapping the random.randint function
    returns a random integer between the given range
    """

    traits = frozenset({lowering.FromPythonCall()})
    start: ir.SSAValue = info.argument(types.Int)
    stop: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(types.Int)


@statement(dialect=dialect)
class Uniform(ir.Statement):
    """uniform statement, wrapping the random.uniform function
    returns a random floating number between the given range
    """

    traits = frozenset({lowering.FromPythonCall()})
    start: ir.SSAValue = info.argument(types.Float)
    stop: ir.SSAValue = info.argument(types.Float)
    result: ir.ResultValue = info.result(types.Float)


@statement(dialect=dialect)
class Seed(ir.Statement):
    """seed statement, wrapping the random.seed function
    sets the seed for the random number generator
    """

    traits = frozenset({lowering.FromPythonCall()})
    value: ir.SSAValue = info.argument(types.Int)
