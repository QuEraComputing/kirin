import pytest

from kirin.dialects.py import stmts
from kirin.ir import Block


def test_stmt():
    a = stmts.Constant(0)
    x = stmts.Constant(1)
    y = stmts.Constant(2)
    z = stmts.Add(lhs=x.result, rhs=y.result)

    bb1 = Block([a, x, y, z])
    assert bb1.first_stmt == a
    bb1.print()

    a.delete()
    assert bb1.first_stmt == x
    bb1.print()

    a.insert_before(x)
    bb1.print()
    assert bb1.first_stmt == a

    a.delete()
    a.insert_after(x)
    bb1.stmts.at(1) == a  # type: ignore

    with pytest.raises(ValueError):
        a.insert_after(x)

    with pytest.raises(ValueError):
        a.insert_before(x)


def test_block_append():
    block = Block()
    block.stmts.append(stmts.Constant(1))
    block.stmts.append(stmts.Constant(1))
    block.print()
    assert len(block.stmts) == 2
