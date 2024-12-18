from kirin.prelude import basic
from kirin.dialects import fcf


@basic(fold=False, typeinfer=True)
def enumerate_kirin(arr):
    return fcf.Collect(range(len(arr)))


def test_enumerate_kirin():
    assert enumerate_kirin([1, 2, 3, 4, 5]) == list(range(5))