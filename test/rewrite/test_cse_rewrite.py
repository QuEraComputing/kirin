from dataclasses import dataclass

from kirin import ir, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt
from kirin.dialects.py.constant import Constant
from kirin.rewrite.cse import _HASHABLE_SLICE, Info, CommonSubexpressionElimination
from kirin.rewrite.walk import Walk

dialect = ir.Dialect("test")


@dataclass
class Unhashable:
    """A default dataclass instance is unhashable (eq=True sets __hash__=None)."""

    x: int = 1


@statement(dialect=dialect)
class MultiResult(ir.Statement):
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    result_a: ir.ResultValue = info.result()
    result_b: ir.ResultValue = info.result()


dummy_dialect = basic_no_opt.add(dialect)


def test_multi_result():
    @dummy_dialect
    def duplicated():
        x, y = MultiResult()  # type: ignore
        a, b = MultiResult()  # type: ignore
        return x + a, y + b

    stmt_0 = duplicated.callable_region.blocks[0].stmts.at(0)
    stmt_1 = duplicated.callable_region.blocks[0].stmts.at(1)
    assert isinstance(stmt_0, MultiResult)
    assert isinstance(stmt_1, MultiResult)

    Walk(CommonSubexpressionElimination()).rewrite(duplicated.code)

    stmt_0 = duplicated.callable_region.blocks[0].stmts.at(0)
    stmt_1 = duplicated.callable_region.blocks[0].stmts.at(1)
    assert isinstance(stmt_0, MultiResult)
    assert not isinstance(stmt_1, MultiResult)


def test_info():
    info_value = Info(ir.Statement, (), (ir.PyAttr(slice(None)),), (), ())

    if not _HASHABLE_SLICE:
        assert info_value._hashable is False
        assert info_value._hash == id(info_value)
    else:
        assert info_value._hashable is True
        info_value._hash = hash((ir.Statement,) + (ir.PyAttr(slice(None)),))


def test_info_unhashable_pyattr():
    # issue: CSE must not crash when a PyAttr wraps an arbitrary unhashable
    # value (the general case of the slice handling above). Such an Info is
    # simply treated as non-hashable so its statement never CSE-merges.
    obj = Unhashable()
    info_value = Info(Constant, (), (ir.PyAttr(obj),), (), ())

    assert info_value._hashable is False
    assert info_value._hash == id(info_value)
    # hashing the Info itself must not raise even though the data is unhashable
    assert hash(info_value) == id(info_value)


def test_cse_unhashable_constants():
    # End-to-end: two py.constant statements wrapping the same unhashable
    # object must flow through CSE without raising TypeError.
    obj = Unhashable()

    block = ir.Block()
    block.stmts.append(Constant(obj))
    block.stmts.append(Constant(obj))

    result = CommonSubexpressionElimination().rewrite_Block(block)

    # The two unhashable constants are conservatively left un-merged.
    assert result.has_done_something is False
    assert len(block.stmts) == 2
