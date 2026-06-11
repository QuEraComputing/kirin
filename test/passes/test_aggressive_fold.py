"""Tests for the ``inline_heuristic`` parameter of ``kirin.passes.aggressive.Fold``.

``Fold`` historically inlined every callee (``Inline(lambda _: True)``). The
``inline_heuristic`` parameter lets a caller fold/inline everything *except*
selected callees, leaving them as swappable ``func.Invoke`` holes -- while still
folding the rest of the program. Default behavior is unchanged.
"""

import pytest

from kirin.prelude import basic
from kirin.dialects import func
from kirin.passes.aggressive import Fold


@basic(typeinfer=True)
def inc(x: int) -> int:
    return x + 1


@basic(typeinfer=True)
def dbl(x: int) -> int:
    return x * 2


@basic(typeinfer=True)
def caller(x: int) -> int:
    # caller(x) == dbl(inc(x)) + 1 == 2*(x+1) + 1 == 2x + 3
    return dbl(inc(x)) + 1


def _invoked_syms(mt) -> list[str]:
    return sorted(
        s.callee.sym_name
        for blk in mt.callable_region.blocks
        for s in blk.stmts
        if isinstance(s, func.Invoke)
    )


def _folded(heuristic):
    mt = caller.similar()
    Fold(mt.dialects, inline_heuristic=heuristic).fixpoint(mt)
    return mt


# --- default / backward compatibility -------------------------------------

def test_default_inlines_everything():
    """No inline_heuristic -> historical behavior: every callee inlined away."""
    mt = caller.similar()
    Fold(mt.dialects).fixpoint(mt)          # positional construction unchanged
    assert _invoked_syms(mt) == []


def test_explicit_inline_all_matches_default():
    assert _invoked_syms(_folded(lambda _: True)) == []


# --- selective inlining ----------------------------------------------------

def test_hold_one_callee_out():
    """Everything except the named callee is inlined; it stays an Invoke hole."""
    mt = _folded(lambda code: code.sym_name != "inc")
    assert _invoked_syms(mt) == ["inc"]     # dbl inlined, inc held out


def test_hold_multiple_callees_out():
    mt = _folded(lambda code: code.sym_name not in {"inc", "dbl"})
    assert _invoked_syms(mt) == ["dbl", "inc"]


def test_inline_nothing():
    """A heuristic that never inlines leaves all calls in place."""
    mt = _folded(lambda _: False)
    assert _invoked_syms(mt) == ["dbl", "inc"]


def test_folding_still_happens_around_a_hole():
    """Holding one callee out must NOT disable folding of the rest: the *other*
    callee is still inlined while only the held-out one survives as a hole."""
    held = _folded(lambda code: code.sym_name != "inc")   # inline dbl, hold inc
    none = _folded(lambda _: False)                        # inline nothing
    # 'dbl' is inlined (gone) even though 'inc' is held out:
    assert _invoked_syms(held) == ["inc"]
    assert _invoked_syms(none) == ["dbl", "inc"]


# --- semantics preservation ------------------------------------------------

@pytest.mark.parametrize(
    "heuristic",
    [
        pytest.param(lambda _: True, id="inline-all"),
        pytest.param(lambda _: False, id="inline-none"),
        pytest.param(lambda code: code.sym_name != "inc", id="hold-inc"),
        pytest.param(lambda code: code.sym_name not in {"inc", "dbl"}, id="hold-both"),
    ],
)
def test_semantics_preserved(heuristic):
    mt = _folded(heuristic)
    for x in (-7, -1, 0, 4, 23):
        assert mt(x) == caller(x)           # == 2x + 3, regardless of heuristic


# --- the two-phase use case (hold out, then inline later) ------------------

def test_held_out_hole_can_be_inlined_in_a_later_pass():
    """The intended pattern: fold the scaffold once holding a callee out, then
    later inline just that callee (e.g. after swapping it in)."""
    mt = caller.similar()
    Fold(mt.dialects, inline_heuristic=lambda code: code.sym_name != "inc").fixpoint(mt)
    assert _invoked_syms(mt) == ["inc"]
    # later: inline only the previously-held-out callee
    Fold(mt.dialects, inline_heuristic=lambda code: code.sym_name == "inc").fixpoint(mt)
    assert _invoked_syms(mt) == []
    for x in (-7, 0, 4, 23):
        assert mt(x) == caller(x)
