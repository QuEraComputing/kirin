from attrs import Food, Serving
from dialect import dialect

from kirin import ir, types
from kirin.decl import info, statement


@statement(dialect=dialect)
class NewFood(ir.Statement):
    name = "new_food"
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    type: str = info.attribute(types.String)
    result: ir.ResultValue = info.result(types.PyClass(Food))


@statement(dialect=dialect)
class Cook(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    target: ir.SSAValue = info.argument(types.PyClass(Food))
    amount: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(types.PyClass(Serving))


@statement(dialect=dialect)
class Eat(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    target: ir.SSAValue = info.argument(types.PyClass(Serving))


@statement(dialect=dialect)
class Nap(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})


@statement(dialect=dialect)
class RandomBranch(ir.Statement):
    name = "random_br"
    traits = frozenset({ir.IsTerminator()})
    cond: ir.SSAValue = info.argument(types.Bool)
    then_arguments: tuple[ir.SSAValue, ...] = info.argument()
    else_arguments: tuple[ir.SSAValue, ...] = info.argument()
    then_successor: ir.Block = info.block()
    else_successor: ir.Block = info.block()
