import pytest

from kirin.passes import Specialize
from kirin.prelude import python_no_opt as kernel
from kirin.dialects import func
from kirin.dialects.py.list import Append

# Keep calls observable so folding cannot erase the specialization under test.
out: list = []


@pytest.fixture(autouse=True)
def _reset_out():
    out.clear()


def get_invokes(method):
    return [stmt for stmt in method.code.walk() if isinstance(stmt, func.Invoke)]


def test_bound_lambdas_preserve_distinct_captures():
    @kernel
    def make_adder(offset: int):
        def add(x: int, y: int):
            Append(out, offset + x + y)

        return add

    first = make_adder(10)
    second = make_adder(20)
    assert first is not second
    assert first.code is second.code
    captures = first.code.captured
    capture_uses = [set(value.uses) for value in captures]

    @kernel
    def root(y: int):
        first(3, y)
        second(3, y)

    Specialize(root.dialects, no_raise=False)(root)
    clones = [call.callee for call in get_invokes(root)]
    assert clones[0] is not clones[1]
    for clone, original in zip(clones, (first, second)):
        assert isinstance(clone.code, func.Lambda)
        assert clone.code.captured == ()
        assert clone.fields is original.fields
        assert clone.code.slots == ("y",)
        clone.verify_type()
    assert first.code.captured == captures
    assert [set(value.uses) for value in captures] == capture_uses
    root(4)
    assert out == [17, 27]


def test_known_local_lambda_call_becomes_specialized_invoke():
    @kernel
    def root(y: int):
        offset = 10

        def known_local_add(x: int, z: int):
            Append(out, offset + x + z)

        known_local_add(3, y)

    assert any(isinstance(stmt, func.Call) for stmt in root.code.walk())
    Specialize(root.dialects, no_raise=False)(root)
    (call,) = get_invokes(root)
    assert isinstance(call.callee.code, func.Lambda)
    assert call.callee.code.captured == ()
    assert call.callee.code.slots == ("z",)
    call.callee.verify_type()
    root(4)
    assert out == [17]


def test_lambda_with_dynamic_captures_remains_a_call():
    @kernel
    def root(offset: int, y: int):
        def dynamic_local_add(x: int, z: int):
            Append(out, offset + x + z)

        dynamic_local_add(3, y)

    source = next(stmt for stmt in root.code.walk() if isinstance(stmt, func.Lambda))
    Specialize(root.dialects, no_raise=False)(root)
    assert not get_invokes(root)
    (call,) = [stmt for stmt in root.code.walk() if isinstance(stmt, func.Call)]
    assert call.callee.owner is source
    assert source.slots == ("x", "z")
    root(10, 4)
    root(20, 4)
    assert out == [17, 27]


def test_captured_object_is_preserved_without_a_specialization_key():
    @kernel
    def factory(values: list):
        def record(x: int):
            Append(out, values)
            Append(out, x)

        return record

    values = [1]
    record = factory(values)

    @kernel
    def root():
        record(3)

    Specialize(root.dialects, no_raise=False)(root)
    clone = get_invokes(root)[0].callee
    assert clone.code.slots == ()
    assert clone.fields[0] is values
    values.append(2)
    root()
    assert out[0] is values
    assert out == [[1, 2], 3]
