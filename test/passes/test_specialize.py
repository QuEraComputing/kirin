import pytest

from kirin.passes import Specialize
from kirin.prelude import python_no_opt as kernel
from kirin.dialects import func
from kirin.dialects.ilist import IList
from kirin.dialects.py.list import Append
from kirin.dialects.py.constant import Constant

# `Append` is impure, so calls that reach it survive the folding the pass runs
# after each visit. Kernels record into `out`; tests assert on it directly.
out: list = []


@pytest.fixture(autouse=True)
def _reset_out():
    out.clear()


def get_invokes(method):
    return [stmt for stmt in method.code.walk() if isinstance(stmt, func.Invoke)]


def test_constant_folding():
    @kernel
    def foo(a: int, x: int, y: float):
        Append(out, a + (x * 10 + y))

    @kernel
    def root(a: int):
        foo(a, 3, 4.0)

    Specialize(root.dialects, no_raise=False)(root)
    specialized = get_invokes(root)[0].callee
    assert any(
        isinstance(stmt, Constant) and stmt.value.data == 34.0
        for stmt in specialized.code.walk()
    )


def test_original_unchanged():
    @kernel
    def foo(a: int, x: int):
        Append(out, a + x)

    @kernel
    def root(a: int):
        foo(a, 3)

    original = foo.similar()
    Specialize(root.dialects, no_raise=False)(root)
    assert get_invokes(root)[0].callee is not foo
    assert foo.is_structurally_equal(original)


def test_reapplication_keeps_clone():
    @kernel
    def foo(a: int, x: int):
        Append(out, a + x)

    @kernel
    def root(a: int):
        foo(a, 3)

    specialize = Specialize(root.dialects, no_raise=False)
    specialize(root)
    specialized = get_invokes(root)[0].callee
    assert specialized is not foo

    specialize(root)
    assert get_invokes(root)[0].callee is specialized


def test_computed_constant_and_nested_calls():
    @kernel
    def inner(a: int, x: int):
        Append(out, a + x)

    @kernel
    def outer(a: int, x: int):
        inner(a, x * 2)

    @kernel
    def root(a: int):
        outer(a, 1 + 2)

    Specialize(root.dialects, no_raise=False)(root)
    outer_clone = get_invokes(root)[0].callee
    inner_clone = get_invokes(outer_clone)[0].callee
    assert outer_clone is not outer
    assert inner_clone is not inner
    assert outer_clone.nargs == inner_clone.nargs == 2
    root(10)
    assert out == [16]


def test_budget_and_zero_budget():
    @kernel
    def foo(a: int, x: int):
        Append(out, a + x)

    @kernel
    def root(a: int):
        foo(a, 1)
        foo(a, 2)
        foo(a, 1)

    before = root.similar()
    assert not Specialize(root.dialects, max_specializations=0)(root).has_done_something
    assert root.is_structurally_equal(before)
    Specialize(root.dialects, max_specializations=1, no_raise=False)(root)
    calls = get_invokes(root)
    assert calls[0].callee is calls[2].callee
    assert calls[1].callee is foo
    root(10)
    assert out == [11, 12, 11]
    with pytest.raises(ValueError):
        Specialize(root.dialects, max_specializations=-1)


def test_recursive_call_reuses_clone():
    @kernel
    def rec(n: int, x: int):
        Append(out, x)
        if n > 0:
            rec(n - 1, 3)

    @kernel
    def root(n: int):
        rec(n, 3)

    # Existing recursive constant analysis may fail conservatively; literal
    # arguments still allow the bounded rewrite to specialize this program.
    Specialize(root.dialects, max_specializations=2)(root)
    rec_specialized = get_invokes(root)[0].callee
    assert rec_specialized is not rec
    assert rec_specialized.nargs == rec.nargs - 1 == 2
    assert all(call.callee is rec_specialized for call in get_invokes(rec_specialized))
    root(2)
    assert out == [3, 3, 3]


def test_mutable_constants_remain_parameters():
    values = [1]

    @kernel
    def foo(a: int, xs: list, x: int):
        Append(out, a + xs[0] + x)

    @kernel
    def root(a: int):
        foo(a, values, 3)

    Specialize(root.dialects)(root)
    clone = get_invokes(root)[0].callee
    assert clone.code.slots == ("a", "xs")
    root(10)
    values[0] = 2
    root(10)
    assert out == [14, 15]


def test_generic_recursion_keeps_original():
    @kernel
    def countdown(n: int):
        Append(out, n)
        if n > 0:
            countdown(n - 1)

    @kernel
    def root(n: int):
        countdown(2)
        countdown(n)

    Specialize(root.dialects)(root)
    specialized_call, dynamic_call = get_invokes(root)
    assert specialized_call.callee is not countdown
    assert dynamic_call.callee is countdown
    (recursive_call,) = get_invokes(countdown)
    assert recursive_call.callee is countdown
    root(3)
    assert out == [2, 1, 0, 3, 2, 1, 0]


@pytest.mark.parametrize(
    "first, second, shared",
    [
        pytest.param(True, 1, False, id="bool-int"),
        pytest.param(1, 1.0, False, id="int-float"),
        pytest.param(None, None, True, id="none"),
        pytest.param(1.0, 1 + 0j, False, id="float-complex"),
        pytest.param(range(3), range(3), True, id="range-reuse"),
        pytest.param((1, (2, 3)), (1, (2, 3)), True, id="nested-tuple"),
        pytest.param(frozenset((1, 2)), frozenset((2, 1)), True, id="frozenset-order"),
        pytest.param(IList([1, (2, 3)]), IList([1, (2, 3)]), True, id="ilist-reuse"),
        pytest.param((1, 2), IList([1, 2]), False, id="tuple-ilist"),
    ],
)
def test_constant_types_and_cache_reuse(first, second, shared):
    @kernel
    def foo(x):
        Append(out, x)

    @kernel
    def root():
        foo(first)
        foo(second)
        foo(first)

    Specialize(root.dialects)(root)
    calls = get_invokes(root)
    assert len(calls) == 3
    assert calls[0].callee is calls[2].callee
    assert (calls[0].callee is calls[1].callee) is shared
    assert all(call.callee.nargs == 1 and not call.inputs for call in calls)
    root()
    # PyAttr includes the outer value type in its equality.
    assert [type(value) for value in out] == [type(first), type(second), type(first)]
    assert out == [first, second, first]


def test_keyword_call_shares_positional_clone():
    @kernel
    def foo(a: int, b: int):
        return a + b

    @kernel
    def root(x: int):
        # A keyword call that is equivalent to the positional call.
        return foo(3, x) + foo(b=x, a=3)

    Specialize(root.dialects, no_raise=False)(root)
    first, second = get_invokes(root)
    # Call2Invoke aligns keywords to positional before keying, so both sites
    # key to `(3, dyn)` and share one clone rather than making a redundant one.
    assert first.callee.code.slots == ("b",)
    assert second.callee.code.slots == ("b",)
    assert second.callee is first.callee
    assert root(20) == (3 + 20) + (3 + 20)


def test_dynamic_arguments_and_reachable_generic_methods():
    @kernel
    def leaf(a: int, x: int):
        Append(out, a + x)

    @kernel
    def middle(a: int):
        leaf(a, 3)

    @kernel
    def root(a: int):
        middle(a)

    Specialize(root.dialects)(root)
    assert get_invokes(root)[0].callee is middle
    assert get_invokes(middle)[0].callee is not leaf
    root(10)
    assert out == [13]


def test_returned_self_denotes_the_original():
    @kernel
    def me(x: int):
        Append(out, x)
        return me

    @kernel
    def root():
        return me(3)

    Specialize(root.dialects, no_raise=False)(root)
    clone = get_invokes(root)[0].callee
    assert clone is not me
    assert clone.nargs == 1
    assert me.nargs == 2
    # Self inside a specialized body still denotes the original callable.
    assert root() is me
    assert out == [3]


def test_return_value_and_branch_specialization():
    @kernel
    def choose(a: int, yes: bool):
        if yes:
            return a + 1
        else:
            return a - 1

    @kernel
    def root(a: int):
        return choose(a, True) + choose(a, False)

    before = [root(i) for i in (-10, 0, 5)]
    Specialize(root.dialects, no_raise=False)(root)
    assert [root(i) for i in (-10, 0, 5)] == before
    assert len(get_invokes(root)) == 2
    for call in get_invokes(root):
        assert call.callee.nargs == 2
        call.callee.verify_type()


def test_specialization_preserves_effectful_argument_evaluation():
    @kernel
    def argument(a: int):
        Append(out, a)
        return 3

    @kernel
    def consumer(a: int, x: int):
        Append(out, a + x)

    @kernel
    def root(a: int):
        consumer(a, argument(a))

    Specialize(root.dialects, no_raise=False)(root)
    calls = get_invokes(root)
    assert calls[0].callee is argument
    assert calls[1].callee is not consumer
    assert len(calls[1].inputs) == 1
    # Baking in argument's return value must not eliminate its side effect.
    root(10)
    assert out == [10, 13]
