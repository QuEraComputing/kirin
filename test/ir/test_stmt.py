import pytest

from kirin.ir import Block
from kirin.source import SourceInfo
from kirin.dialects import py


def test_stmt():
    a = py.Constant(0)
    x = py.Constant(1)
    y = py.Constant(2)
    z = py.Add(lhs=x.result, rhs=y.result)

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
    block.stmts.append(py.Constant(1))
    block.stmts.append(py.Constant(1))
    block.print()
    assert len(block.stmts) == 2


def test_stmt_from_stmt():

    x = py.Constant(1)

    x.result.hints["const"] = py.constant.types.Int

    y = x.from_stmt(x)

    assert y.result.hints["const"] == py.constant.types.Int


def test_stmt_from_stmt_preserves_source_info():
    x = py.Constant(1)
    x.source = SourceInfo(lineno=1, col_offset=0, end_lineno=None, end_col_offset=None)

    y = x.from_stmt(x)
    assert y.source == x.source
    assert y.source is x.source
