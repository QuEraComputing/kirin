# type: ignore
from kirin.prelude import python_no_opt
from kirin.dialects import py


def test_list_append():

    @python_no_opt
    def test_append():
        x = []
        py.Append(x, 1)
        py.Append(x, 2)
        return x

    y = test_append()

    assert len(y) == 2
    assert y[0] == 1
    assert y[1] == 2


def test_recursive_append():
    @python_no_opt
    def for_loop_append(cntr: int, x: list, n_range: int):
        if cntr < n_range:
            py.Append(x, cntr)
            for_loop_append(cntr + 1, x, n_range)

        return x

    assert for_loop_append(0, [], 5) == [0, 1, 2, 3, 4]
