# type: ignore
from kirin.prelude import basic
from kirin.dialects.py import stmts


def test_list_append():

    @basic
    def test_append():
        x = []
        stmts.Append(x, 1)
        stmts.Append(x, 2)
        return x

    y = test_append()

    assert len(y) == 2
    assert y[0] == 1
    assert y[1] == 2


def test_recursive_append():
    @basic
    def for_loop_append(cntr: int, x: list, n_range: int):
        if cntr < n_range:
            stmts.Append(x, cntr)
            for_loop_append(cntr + 1, x, n_range)

        return x

    assert for_loop_append(0, [], 5) == [0, 1, 2, 3, 4]
