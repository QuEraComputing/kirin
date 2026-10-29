import pytest

from kirin import ir, types
from kirin.decl import info, statement
from kirin.dialects import py

dialect = ir.Dialect("test_isolated")


@statement(dialect=dialect)
class Isolated(ir.Statement):
    traits = frozenset({ir.IsolatedFromAbove()})
    body: ir.Region = info.region(multi=True)
    result: ir.ResultValue = info.result(types.Int)


@statement(dialect=dialect)
class Nested(ir.Statement):
    body: ir.Region = info.region(multi=True)


def test_body_uses_own_values():
    block = ir.Block(argtypes=(types.Int,))
    one = py.Constant(1)
    add = py.Add(block.args[0], one.result)
    block.stmts.append(one)
    block.stmts.append(add)

    Isolated(body=ir.Region(block)).verify()


def test_body_uses_outside_value():
    outside = py.Constant(1)
    block = ir.Block(argtypes=(types.Int,))
    block.stmts.append(py.Add(block.args[0], outside.result))

    with pytest.raises(ir.ValidationError, match="IsolatedFromAbove"):
        Isolated(body=ir.Region(block)).verify()


def test_nested_region_uses_enclosing_value():
    inner = ir.Block(argtypes=(types.Int,))
    outer = ir.Block(argtypes=(types.Int,))
    inner.stmts.append(py.Add(inner.args[0], outer.args[0]))
    outer.stmts.append(Nested(body=ir.Region(inner)))

    Isolated(body=ir.Region(outer)).verify()


def test_nested_region_uses_outside_value():
    outside = py.Constant(1)
    inner = ir.Block(argtypes=(types.Int,))
    inner.stmts.append(py.Add(inner.args[0], outside.result))
    outer = ir.Block()
    outer.stmts.append(Nested(body=ir.Region(inner)))

    with pytest.raises(ir.ValidationError, match="IsolatedFromAbove"):
        Isolated(body=ir.Region(outer)).verify()


def test_lowered_function_verifies():
    from kirin.prelude import basic_no_opt

    @basic_no_opt
    def main(x: int):
        def inner(y: int):
            return x + y

        return inner(1)

    main.verify()


def test_body_uses_own_result():
    block = ir.Block(argtypes=(types.Int,))
    stmt = Isolated(body=ir.Region(block))
    block.stmts.append(py.Add(block.args[0], stmt.result))

    with pytest.raises(ir.ValidationError, match="IsolatedFromAbove"):
        stmt.verify()
