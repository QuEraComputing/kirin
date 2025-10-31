import pytest

from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Chain, Fixpoint, WrapConst
from kirin.analysis import const
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.getitem import InlineGetItem
from kirin.dialects.py.indexing import GetItem


def apply_getitem_optimization(func):
    constprop = const.Propagate(func.dialects)
    frame, _ = constprop.run(func)
    Fixpoint(Walk(WrapConst(frame))).rewrite(func.code)
    inline_getitem = InlineGetItem()
    Fixpoint(Walk(Chain([inline_getitem, DeadCodeElimination()]))).rewrite(func.code)


@pytest.mark.parametrize("index", [0, -1, 1])
def test_getitem_index(index):

    @basic_no_opt
    def func(x: int):
        ylist = (x, x, 1, x)
        return ylist[index]

    before = func(1)
    apply_getitem_optimization(func)
    after = func(1)

    assert before == after
    assert len(func.callable_region.blocks[0].stmts) == 1


@pytest.mark.parametrize(
    "sl",
    [
        slice(0, 2, 1),
        slice(None, None, None),
        slice(None, -1, None),
        slice(-1, None, None),
        slice(None, None, -1),
        slice(1, 4, 2),
    ],
)
def test_getitem_slice(sl):

    @basic_no_opt
    def func():
        ylist = (0, 1, 2, 3, 4)
        return ylist[sl]

    stmt_types = [type(stmt) for stmt in func.callable_region.blocks[0].stmts]
    assert GetItem in stmt_types

    before = func()
    apply_getitem_optimization(func)
    after = func()

    assert before == after
    stmt_types = [type(stmt) for stmt in func.callable_region.blocks[0].stmts]
    assert GetItem not in stmt_types


@pytest.mark.parametrize(
    "start, stop, step",
    [
        (0, 2, 1),
        (None, None, None),
        (None, -1, None),
        (-1, None, None),
        (None, None, -1),
        (1, 4, 2),
    ],
)
def test_getitem_slice_with_literal_indices(start, stop, step):

    @basic_no_opt
    def func():
        ylist = (0, 1, 2, 3, 4)
        return ylist[start:stop:step]

    stmt_types = [type(stmt) for stmt in func.callable_region.blocks[0].stmts]
    assert GetItem in stmt_types

    before = func()

    apply_getitem_optimization(func)

    stmt_types = [type(stmt) for stmt in func.callable_region.blocks[0].stmts]
    assert GetItem not in stmt_types
    after = func()

    assert before == after


def test_getitem_slice_does_nothing():
    """Test that inline_getiem returns `has_done_something=False` when the getitem
    statement has no uses"""

    @basic_no_opt
    def func():
        ylist = (0, 1, 2, 3, 4)
        ylist[0]  # no use
        return 1

    constprop = const.Propagate(func.dialects)
    frame, _ = constprop.run(func)
    Fixpoint(Walk(WrapConst(frame))).rewrite(func.code)

    inline_getitem = InlineGetItem()

    assert not Walk(inline_getitem).rewrite(func.code).has_done_something
