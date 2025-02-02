"""This module provides access to Python iterables.

This is used to lower Python loops into `cf` dialect.
"""

from ast import Call

from kirin import ir, interp, lowering
from kirin.decl import info, statement
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("py.iterable")

PyRangeIterType = ir.types.PyClass(type(iter(range(0))))


@statement(dialect=dialect)
class Iter(ir.Statement):
    """This is equivalent to `iter(value)` in Python."""

    traits = frozenset({ir.Pure()})
    value: ir.SSAValue = info.argument(ir.types.Any)
    iter: ir.ResultValue = info.result(ir.types.Any)


@statement(dialect=dialect)
class Next(ir.Statement):
    """This is equivalent to `next(iterable, None)` in Python."""

    iter: ir.SSAValue = info.argument(ir.types.Any)
    value: ir.ResultValue = info.result(ir.types.Any)


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Iter)
    def iter_(self, interp, frame: interp.Frame, stmt: Iter):
        return (iter(frame.get(stmt.value)),)

    @interp.impl(Next)
    def next_(self, interp, frame: interp.Frame, stmt: Next):
        return (next(frame.get(stmt.iter), None),)


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(Iter, ir.types.PyClass(range))
    def iter_(self, interp, frame: interp.Frame, stmt: Iter):
        return (PyRangeIterType,)

    @interp.impl(Next, PyRangeIterType)
    def next_(self, interp, frame: interp.Frame, stmt: Next):
        return (ir.types.Int,)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Call_iter(
        self, state: lowering.LoweringState, node: Call
    ) -> lowering.Result:
        if len(node.args) != 1:
            raise DialectLoweringError("iter() takes exactly 1 argument")
        return lowering.Result(
            state.append_stmt(Iter(state.visit(node.args[0]).expect_one()))
        )

    def lower_Call_next(
        self, state: lowering.LoweringState, node: Call
    ) -> lowering.Result:
        if len(node.args) == 2:
            raise DialectLoweringError(
                "next() does not throw StopIteration inside kernel"
            )
        if len(node.args) != 1:
            raise DialectLoweringError("next() takes exactly 1 argument")
        return lowering.Result(
            state.append_stmt(Next(state.visit(node.args[0]).expect_one()))
        )
