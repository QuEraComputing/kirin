from kirin.prelude import basic
from kirin.dialects import fcf


@basic(fold=False, typeinfer=True)
def enumerate_kirin(arr):
    def map_func(i):
        return (i, arr[i])

    return fcf.Map(map_func, range(len(arr)))


def test_enumerate_kirin():
    assert enumerate_kirin([1, 2, 3, 4, 5]) == ((0, 1), (1, 2), (2, 3), (3, 4), (4, 5))
