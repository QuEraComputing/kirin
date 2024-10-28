from kirin.analysis.cfg import CFG
from kirin.prelude import basic_no_opt


@basic_no_opt
def deadblock(x):
    if x:
        return x + 1
    else:
        return x + 2
    return x + 3


def test_reachable():
    cfg = CFG(deadblock.callable_region)
    assert deadblock.code.body.blocks[-1] not in cfg.successors  # type: ignore


@basic_no_opt
def foo(x: int):  # type: ignore
    def goo(y: int):
        return x + y

    return goo


def test_foo_cfg():
    cfg = CFG(foo.callable_region)
    assert foo.callable_region.blocks[0] in cfg.successors
    assert foo.callable_region.blocks[1] not in cfg.successors
