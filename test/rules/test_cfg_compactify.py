from kirin.analysis.cfg import CFG
from kirin.dialects.func import Lambda
from kirin.prelude import basic_no_opt
from kirin.rewrite import Fixpoint
from kirin.rules.cfg_compatify import CFGCompactify


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
