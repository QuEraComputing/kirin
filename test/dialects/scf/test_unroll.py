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
