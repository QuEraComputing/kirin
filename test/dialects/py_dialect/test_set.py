from kirin.prelude import basic


@basic
def make_set():
    return {1, 1, 2}


@basic
def make_empty_set():
    return set()


def test_set_runtime_result():
    out = make_set()
    assert isinstance(out, set)
    assert out == {1, 2}


def test_empty_set_runtime_result():
    out = make_empty_set()
    assert isinstance(out, set)
    assert out == set()
