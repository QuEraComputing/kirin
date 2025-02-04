from attrs import Beer, Pints
from dialect import dialect

from kirin import ir, types
from kirin.decl import info, statement


@statement(dialect=dialect)
class NewBeer(ir.Statement):
    name = "new_beer"
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    brand: str = info.attribute(types.String)
    result: ir.ResultValue = info.result(types.PyClass(Beer))


@statement(dialect=dialect)
class Pour(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    beverage: ir.SSAValue = info.argument(types.PyClass(Beer))
    amount: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(types.PyClass(Pints))


@statement(dialect=dialect)
class Drink(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    pints: ir.SSAValue = info.argument(types.PyClass(Pints))


@statement(dialect=dialect)
class Puke(ir.Statement):
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
