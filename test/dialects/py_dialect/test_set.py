from kirin.prelude import basic, structural
from kirin.dialects import py


@basic
def make_set():
    return {1, 1, 2}


@basic
def make_empty_set():
    return set()


@basic
def comp_simple():
    return {x for x in range(3)}


@basic
def comp_filtered():
    return {x for x in range(5) if x % 2 == 0}


@structural
def comp_nested():
    return {(x, y) for x in range(2) for y in range(3) if y}


@basic
def comp_dedup():
    return {x % 2 for x in range(5)}


@basic
def comp_temp_name_collision():
    _kirin_setcomp_tmp = 99
    return _kirin_setcomp_tmp, {x for x in range(2)}


@basic.add(py.unpack)
def comp_unpacking():
    pairs = [(1, 2), (3, 4)]
    return {a + b for a, b in pairs}


def test_set_runtime_result():
    out = make_set()
    assert isinstance(out, set)
    assert out == {1, 2}


def test_empty_set_runtime_result():
    out = make_empty_set()
    assert isinstance(out, set)
    assert out == set()


def test_set_comp_runtime_simple():
    assert comp_simple() == {0, 1, 2}


def test_set_comp_runtime_filtered():
    assert comp_filtered() == {0, 2, 4}


def test_set_comp_runtime_nested():
    assert comp_nested() == {(0, 1), (0, 2), (1, 1), (1, 2)}


def test_set_comp_runtime_dedup():
    assert comp_dedup() == {0, 1}


def test_set_comp_temp_name_collision():
    assert comp_temp_name_collision() == (99, {0, 1})


def test_set_comp_unpacking():
    assert comp_unpacking() == {3, 7}
