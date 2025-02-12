from kirin.passes import Fold
from kirin.prelude import basic, structural_no_opt


def test_simple_loop():
    @structural_no_opt
    def simple_loop():
        x = 0
        for i in range(2):
            x = x + 1
        return x

    @basic(fold=True)
    def target():
        return 2

    fold = Fold(structural_no_opt)
    fold(simple_loop)
    assert target.callable_region.is_equal(simple_loop.callable_region)
