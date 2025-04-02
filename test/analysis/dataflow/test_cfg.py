from kirin import lowering
from kirin.prelude import basic_no_opt
from kirin.dialects import func
from kirin.analysis.cfg import CFG

lower = lowering.Python(basic_no_opt)


def deadblock(x):
    if x:
        return x + 1
    else:
        return x + 2
    return x + 3


def test_reachable():
    code = lower.python_function(deadblock, compactify=False)
    assert isinstance(code, func.Function)
    cfg = CFG(code.body)
    assert code.body.blocks[-1] not in cfg.successors


def foo(x: int):  # type: ignore
    def goo(y: int):
        return x + y

    return goo


def test_foo_cfg():
    code = lower.python_function(foo, compactify=False)
    assert isinstance(code, func.Function)
    cfg = CFG(code.body)
    assert code.body.blocks[0] in cfg.successors
    assert code.body.blocks[1] not in cfg.successors
