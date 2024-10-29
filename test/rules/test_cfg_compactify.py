from kirin.analysis.cfg import CFG
from kirin.dialects import func
from kirin.dialects.func import Lambda
from kirin.dialects.py import stmts
from kirin.prelude import basic_no_opt
from kirin.rewrite import Fixpoint, Walk
from kirin.rules.cfg_compatify import CFGCompactify
from kirin.rules.inline import Inline


@basic_no_opt
def foo(x: int):  # type: ignore
    def goo(y: int):
        return x + y

    return goo


def test_cfg_compactify():
    cfg = CFG(foo.callable_region)
    compactify = CFGCompactify(cfg)
    Fixpoint(compactify).rewrite(foo.code)
    foo.callable_region.blocks[0].stmts.at(1).print()
    assert len(foo.callable_region.blocks[0].stmts) == 2
    stmt = foo.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, Lambda)
    assert len(stmt.body.blocks[0].stmts) == 3
    assert len(stmt.body.blocks) == 1


@basic_no_opt
def my_func(x: int, y: int):
    def foo(a: int, b: int):
        return a + b + x + y

    return foo


@basic_no_opt
def my_main_test_cfg():
    a = 3
    b = 4
    c = my_func(1, 2)
    return c(a, b) * 4


def test_compactify_replace_block_arguments():
    Walk(Inline(heuristic=lambda x: True)).rewrite(my_main_test_cfg.code)
    cfg = CFG(my_main_test_cfg.callable_region)
    compactify = CFGCompactify(cfg)
    Fixpoint(compactify).rewrite(my_main_test_cfg.code)
    my_main_test_cfg.code.print()
    stmt = my_main_test_cfg.callable_region.blocks[0].stmts.at(5)
    assert isinstance(stmt, func.Lambda)
    assert isinstance(stmt.captured[0].owner, stmts.Constant)
    assert stmt.captured[0].name == "x"
    assert isinstance(stmt.captured[1].owner, stmts.Constant)
    assert stmt.captured[1].name == "y"
