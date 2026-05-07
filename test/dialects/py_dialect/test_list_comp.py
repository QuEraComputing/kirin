from kirin.prelude import basic, structural, structural_no_opt
from kirin.dialects import py, ilist


@basic
def simple():
    return [x for x in range(3)]


@basic
def filtered():
    return [x for x in range(5) if x % 2 == 0]


@structural_no_opt
def nested():
    return [(x, y) for x in range(2) for y in range(3) if y]


@structural
def structural_simple():
    return [x for x in range(3)]


@structural
def structural_nested():
    return [(x, y) for x in range(2) for y in range(3) if y]


@basic
def with_arg(i, j):
    return [(x, y) for x in range(i) for y in range(j) if y]


@basic
def temp_name_collision():
    _kirin_listcomp_tmp = 99
    return _kirin_listcomp_tmp, [x for x in range(2)]


@basic.add(py.unpack)
def unpacking_target():
    pairs = [(1, 2), (3, 4)]
    return [a + b for a, b in pairs]


def test_with_arg():
    assert with_arg(2, 3) == ilist.IList([(0, 1), (0, 2), (1, 1), (1, 2)])


def test_simple_runtime():
    assert simple() == ilist.IList([0, 1, 2])


def test_filtered_runtime():
    assert filtered() == ilist.IList([0, 2, 4])


def test_nested_runtime():
    assert nested() == ilist.IList([(0, 1), (0, 2), (1, 1), (1, 2)])


def test_structural_simple_runtime():
    assert structural_simple() == ilist.IList([0, 1, 2])


def test_structural_nested_runtime():
    assert structural_nested() == ilist.IList([(0, 1), (0, 2), (1, 1), (1, 2)])


def test_temp_name_collision():
    assert temp_name_collision() == (99, ilist.IList([0, 1]))


def test_unpacking_target():
    assert unpacking_target() == ilist.IList([3, 7])
