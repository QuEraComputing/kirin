from kirin.analysis.cfg import CFG
from kirin.dialects.func import Lambda
from kirin.interp import Interpreter
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
    return x + y


def test_cfg_compactify2():

    @basic_no_opt
    def my_main_test_cfg():

        a = 3
        b = 4
        a2 = 5
        b2 = 6
        c = my_func(a, b)
        c2 = my_func(a2, b2)
        return c * 4, c2

    # first inline:
    interp = Interpreter(my_main_test_cfg.dialects)
    Walk(Inline(interp=interp, heuristic=lambda x: True)).rewrite(my_main_test_cfg.code)
    my_main_test_cfg.code.print()

    cfg = CFG(my_main_test_cfg.callable_region)
    compactify = CFGCompactify(cfg)
    Fixpoint(compactify).rewrite(my_main_test_cfg.code)
    my_main_test_cfg.code.print()


test_cfg_compactify2()
