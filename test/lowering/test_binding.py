from kirin import lowering
from kirin.prelude import basic_no_opt
from kirin.dialects import math


@lowering.wraps(math.stmts.sin)
def sin(value: float) -> float: ...


@basic_no_opt
def main(x: float):
    return sin(x)


def test_binding():
    stmt = main.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, math.stmts.sin)
