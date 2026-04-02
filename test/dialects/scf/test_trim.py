"""Tests for scf.trim.UnusedYield.

Includes a regression test for a bug where UnusedYield replaces a For
loop's block argument with its initializer when the corresponding result
is unused after the loop, even if the block argument is read inside the
loop body. This breaks loop-carried variable semantics when the previous
iteration's value is needed (e.g., prev = curr pattern).
"""

from kirin import rewrite
from kirin.prelude import structural, python_basic
from kirin.dialects import py, scf, func, ilist, lowering

basic_scf = python_basic.union(
    [func, scf, py.unpack, lowering.func, ilist, lowering.range.ilist]
)


def test_trim_noop_all_results_used():
    """All loop results are used after the loop → nothing to trim (any_unused=False)."""

    @basic_scf
    def main():
        total = 0
        for i in range(4):
            total = total + i
        return total

    expected_return_val = main.py_func()
    assert expected_return_val == 6

    rewrite.Walk(scf.trim.UnusedYield()).rewrite(main.code)

    [loop] = [s for s in main.callable_region.stmts() if isinstance(s, scf.For)]
    assert len(loop.initializers) == 1
    assert len(loop.body.blocks[0].args) == 2

    actual_return_val = main()
    assert actual_return_val == expected_return_val


def test_trim_noop_all_mutated_args_preserved():
    """Results are unused after the loop, but all block args are used and
    mutated inside the body → all get preserved, len(results)==len(node._results)."""

    @basic_scf
    def main():
        a = 0
        b = 1
        for _ in range(5):
            c = a + b
            a = b
            b = c
        return 0

    rewrite.Walk(scf.trim.UnusedYield()).rewrite(main.code)

    [loop] = [s for s in main.callable_region.stmts() if isinstance(s, scf.For)]
    assert len(loop.initializers) == 2
    assert len(loop.body.blocks[0].args) == 3

    actual_return_val = main()
    assert actual_return_val == 0


def test_trim_prev_curr_used_after_loop():
    """curr IS used after the loop → iter_arg preserved. Works correctly."""

    @basic_scf
    def main():
        curr = 0
        for i in range(3):
            prev = curr
            curr = prev + i + 1
        return curr

    expected_return_val = main.py_func()
    assert expected_return_val == 6

    rewrite.Walk(scf.trim.UnusedYield()).rewrite(main.code)
    actual_return_val = main()

    assert actual_return_val == expected_return_val


def test_trim_prev_curr_unused_after_loop():
    """curr is NOT used after the loop → iter_arg incorrectly trimmed.

    BUG: UnusedYield replaces curr's block argument with the initializer (0),
    so `prev = curr` always sees 0 instead of the previous iteration's value.

    Expected (correct Python semantics):
        iter 0: prev=0, curr=1, last_prev=0
        iter 1: prev=1, curr=3, last_prev=1
        iter 2: prev=3, curr=6, last_prev=3
        → return 3

    Actual (after UnusedYield, prev always = 0):
        iter 0: prev=0, curr=1, last_prev=0
        iter 1: prev=0, curr=2, last_prev=0
        iter 2: prev=0, curr=3, last_prev=0
        → return 0
    """

    @basic_scf
    def main():
        curr = 0
        last_prev = 0
        for i in range(3):
            prev = curr
            curr = prev + i + 1
            last_prev = prev
        return last_prev

    expected = main.py_func()
    assert expected == 3

    rewrite.Walk(scf.trim.UnusedYield()).rewrite(main.code)
    actual = main()

    assert actual == expected


def test_trim_with_lists():

    @structural(fold=False, typeinfer=True)
    def mwe():

        result = 0
        start = ilist.IList([0])
        stop = ilist.IList([1])
        for _ in range(10):
            result = start[0] + stop[0]
            start = stop
            stop = ilist.IList([result])

        return result

    expected_return_val = mwe.py_func()
    assert expected_return_val == 89

    rewrite.Walk(scf.trim.UnusedYield()).rewrite(mwe.code)
    actual_return_val = mwe()

    assert actual_return_val == expected_return_val
