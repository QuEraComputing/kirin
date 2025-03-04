import math

from kirin.prelude import basic_no_opt
from kirin.dialects import py


def test_math_pi():
    @basic_no_opt
    def main():
        return math.pi

    stmt = main.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == math.pi
