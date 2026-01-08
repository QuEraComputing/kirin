from kirin.passes import Fold
from kirin.prelude import structural_no_opt
from kirin.rewrite import Walk
from kirin.dialects import py, scf, func


def test_simple_loop_unroll():
    @structural_no_opt
    def simple_loop(x):
        for i in range(3):
            x = x + i
        return x

    fold = Fold(structural_no_opt)
    fold(simple_loop)
    Walk(scf.unroll.ForLoop()).rewrite(simple_loop.code)
    assert len(simple_loop.callable_region.blocks) == 1
    stmts = simple_loop.callable_region.blocks[0].stmts
    assert isinstance(stmts.at(0), py.Constant)
    assert isinstance(stmts.at(1), py.Constant)
    assert isinstance(stmts.at(2), py.Add)
    assert isinstance(stmts.at(3), py.Constant)
    assert isinstance(stmts.at(4), py.Add)
    assert isinstance(stmts.at(5), py.Constant)
    assert isinstance(stmts.at(6), py.Add)
    assert isinstance(stmts.at(7), func.Return)
    assert simple_loop(1) == 4


def test_partial_tuple_loop_unroll():
    @structural_no_opt
    def simple_loop(a: int, b: int, c: int):
        x = 0
        for i in (a, b, c):
            x = x + i
        return x

    fold = Fold(structural_no_opt)
    fold(simple_loop)
    Walk(scf.unroll.ForLoop()).rewrite(simple_loop.code)
    fold(simple_loop)

    # after fold the `getitem` should be eliminated as well
    # leaving just the block arguments being added directly
    # to `x`
    assert len(simple_loop.callable_region.blocks) == 1
    block = simple_loop.callable_region.blocks[0]
    args = block.args
    stmts = block.stmts
    assert isinstance(stmts.at(0), py.Constant)
    assert isinstance(stmt := stmts.at(1), py.Add)
    assert stmt.rhs is args[1]
    assert isinstance(stmt := stmts.at(2), py.Add)
    assert stmt.rhs is args[2]
    assert isinstance(stmt := stmts.at(3), py.Add)
    assert stmt.rhs is args[3]
    assert isinstance(stmts.at(4), func.Return)
