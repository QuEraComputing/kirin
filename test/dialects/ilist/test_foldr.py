from typing import Literal

import pytest

from kirin import types
from kirin.passes import TypeInfer
from kirin.prelude import structural_no_opt
from kirin.rewrite import Walk
from kirin.dialects import ilist


@pytest.mark.parametrize("unroll", [False, True])
def test_foldr_element_precedes_accumulator(unroll: bool) -> None:
    @structural_no_opt
    def step(x: int, acc: tuple[int, int]) -> tuple[int, int]:
        return (10 * acc[0] + x, acc[1] + 1)

    @structural_no_opt
    def folded(xs: ilist.IList[int, Literal[3]]) -> tuple[int, int]:
        return ilist.foldr(step, xs, (0, 0))

    xs = ilist.IList([1, 2, 3], elem=types.Int)
    assert folded(xs) == (321, 3)
    TypeInfer(folded.dialects, no_raise=False)(folded)
    assert folded.return_type == types.Tuple[types.Int, types.Int]
    folded.verify_type()
    if unroll:
        assert Walk(ilist.rewrite.Unroll()).rewrite(folded.code).has_done_something
        folded.verify()
    assert folded(xs) == (321, 3)


@pytest.mark.parametrize("unroll", [False, True])
@pytest.mark.parametrize("initial, expected", [(0, 2), (4, -2)])
def test_foldr_right_associative_subtraction(
    unroll: bool, initial: int, expected: int
) -> None:
    @structural_no_opt
    def subtract(x: int, acc: int) -> int:
        return x - acc

    @structural_no_opt
    def folded(xs: ilist.IList[int, Literal[3]], initial: int) -> int:
        return ilist.foldr(subtract, xs, initial)

    xs = ilist.IList([1, 2, 3], elem=types.Int)
    assert folded(xs, initial) == expected
    TypeInfer(folded.dialects, no_raise=False)(folded)
    folded.verify_type()
    if unroll:
        assert Walk(ilist.rewrite.Unroll()).rewrite(folded.code).has_done_something
        folded.verify()
    assert folded(xs, initial) == expected
