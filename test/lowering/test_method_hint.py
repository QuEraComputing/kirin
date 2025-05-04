from kirin import ir, types
from kirin.prelude import basic


def test_method_type_hint():
    @basic
    def main() -> ir.Method[[int, int], float]:

        def test(x: int, y: int) -> float:
            return x * y * 3.0

        return test

    assert main.return_type == types.Generic(
        ir.Method, [types.Int, types.Int], types.Float
    )
