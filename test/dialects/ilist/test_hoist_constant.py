"""Tests for lifting ``py.Constant`` through ``ilist`` lambda closures."""

import pytest

from kirin import ir
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Chain, Fixpoint, ConstantFold
from kirin.analysis import const
from kirin.dialects import py, func, ilist
from kirin.rewrite.abc import RewriteResult
from kirin.dialects.ilist.rewrite.hoist_constant import HoistConstant

LOOP_STMTS = (ilist.ForEach, ilist.Map)


@basic_no_opt
def _effect(value: int) -> int:
    """An impure consumer that keeps a constant live in a loop body."""
    return value


def outer_block(mt: ir.Method) -> ir.Block:
    return mt.callable_region.blocks[0]


def loop_stmt(mt: ir.Method) -> ir.Statement:
    loops = [stmt for stmt in outer_block(mt).stmts if isinstance(stmt, LOOP_STMTS)]
    assert len(loops) == 1
    return loops[0]


def body_lambda(mt: ir.Method) -> func.Lambda:
    lam = loop_stmt(mt).fn.owner  # type: ignore[attr-defined]
    assert isinstance(lam, func.Lambda)
    return lam


def body_stmts(mt: ir.Method) -> list[ir.Statement]:
    """Statements in the lambda entry block (most kernels are single-block)."""
    return list(body_lambda(mt).body.blocks[0].stmts)


def all_body_stmts(mt: ir.Method) -> list[ir.Statement]:
    return [
        stmt
        for block in body_lambda(mt).body.blocks
        for stmt in block.stmts
    ]


def apply_hoist(mt: ir.Method) -> RewriteResult:
    result = Fixpoint(Walk(HoistConstant())).rewrite(mt.code)
    mt.verify()
    return result


def foreach_kernel():
    @basic_no_opt
    def mt():
        def body(i):
            _effect(i + 7)

        ilist.for_each(body, ilist.range(3))

    return mt


def map_kernel():
    @basic_no_opt
    def mt(n: int):
        def body(i):
            return i + 7

        return ilist.map(body, ilist.range(0, n, 1))

    return mt


@pytest.mark.parametrize("factory", [foreach_kernel, map_kernel])
def test_hoists_constants_out_of_foreach_and_map(factory):
    mt = factory()
    lam = body_lambda(mt)
    body_constants = [stmt for stmt in body_stmts(mt) if isinstance(stmt, py.Constant)]
    assert body_constants
    n_captured = len(lam.captured)

    result = apply_hoist(mt)

    assert result.has_done_something
    assert not any(isinstance(stmt, py.Constant) for stmt in body_stmts(mt))
    assert len(body_lambda(mt).captured) == n_captured + len(body_constants)


def test_variant_computation_stays_in_body():
    mt = foreach_kernel()

    apply_hoist(mt)

    assert py.binop.Add in [type(stmt) for stmt in body_stmts(mt)]
    assert py.binop.Add not in [type(stmt) for stmt in outer_block(mt).stmts]


def test_captured_invariant_computation_is_not_constant_hoisting():
    @basic_no_opt
    def mt():
        x = 3

        def body(i):
            _effect(x * 2 + i)

        ilist.for_each(body, ilist.range(3))

    apply_hoist(mt)

    # `2` is hoisted, but moving `x * 2` is the later, broader LICM pass.
    assert py.binop.Mult in [type(stmt) for stmt in body_stmts(mt)]


def test_hoisted_value_is_threaded_back_as_a_capture():
    mt = foreach_kernel()
    old_constants = [stmt for stmt in body_stmts(mt) if isinstance(stmt, py.Constant)]

    apply_hoist(mt)

    lam = body_lambda(mt)
    outer_constants = [
        stmt for stmt in outer_block(mt).stmts if isinstance(stmt, py.Constant)
    ]
    assert all(constant in outer_constants for constant in old_constants)
    assert all(constant.result in lam.captured for constant in old_constants)

    fields = [stmt for stmt in body_stmts(mt) if isinstance(stmt, func.GetField)]
    assert fields
    assert all(field.obj is lam.body.blocks[0].args[0] for field in fields)


def test_hoisted_constant_dominates_the_lambda():
    mt = foreach_kernel()
    constants = [stmt for stmt in body_stmts(mt) if isinstance(stmt, py.Constant)]

    apply_hoist(mt)

    block_stmts = list(outer_block(mt).stmts)
    lambda_index = block_stmts.index(body_lambda(mt))
    assert all(block_stmts.index(constant) < lambda_index for constant in constants)


def test_fixpoint_lifts_constants_through_nested_closures():
    @basic_no_opt
    def mt():
        def outer(i):
            def inner(j):
                _effect(i + j + 7)

            ilist.for_each(inner, ilist.range(2))

        ilist.for_each(outer, ilist.range(3))

    outer_body = body_lambda(mt).body.blocks[0]
    inner_loop = next(
        stmt for stmt in outer_body.stmts if isinstance(stmt, ilist.ForEach)
    )
    inner_lambda = inner_loop.fn.owner
    assert isinstance(inner_lambda, func.Lambda)
    assert any(
        isinstance(stmt, py.Constant)
        for block in inner_lambda.body.blocks
        for stmt in block.stmts
    )

    apply_hoist(mt)

    # The inner rewrite first moves its constants into the outer closure. A
    # later fixpoint iteration then moves them through that closure as well.
    inner_lambda = inner_loop.fn.owner
    assert isinstance(inner_lambda, func.Lambda)
    assert not any(isinstance(stmt, py.Constant) for stmt in outer_body.stmts)
    assert not any(
        isinstance(stmt, py.Constant)
        for block in inner_lambda.body.blocks
        for stmt in block.stmts
    )


def test_constant_folded_statement_is_hoisted():
    """A constant that only appears mid-fixpoint, produced by another rule.

    ``const.Propagate`` analyses a closure body in a throwaway frame -- see
    ``ilist/constprop.py``, where ``detect_purity`` calls ``constprop.call``
    and keeps only the purity verdict -- so ``WrapConst`` never annotates
    values inside a lambda and ``ConstantFold`` cannot fire in there on its
    own. The hint is seeded by hand: it is what a const analysis that
    descended into closures would have produced for ``1 + 2``.
    """

    @basic_no_opt
    def mt(n: int):
        def body(i):
            y = 1 + 2
            return y + i

        return ilist.map(body, ilist.range(0, n, 1))

    before = mt(4)

    def threes(stmts):
        return [
            stmt
            for stmt in stmts
            if isinstance(stmt, py.Constant) and stmt.value.unwrap() == 3
        ]

    # `1 + 2` lowers to two constants and an Add, so nothing holds 3 yet: the
    # constant under test can only come from the fold.
    assert not threes(body_stmts(mt)) and not threes(outer_block(mt).stmts)

    foldable = next(
        stmt
        for stmt in body_stmts(mt)
        if isinstance(stmt, py.binop.Add)
        and all(isinstance(arg.owner, py.Constant) for arg in stmt.args)
    )
    foldable.result.hints["const"] = const.Value(3)

    result = Fixpoint(Walk(Chain(ConstantFold(), HoistConstant()))).rewrite(mt.code)
    mt.verify()

    # ConstantFold materialises `py.Constant(3)` inside the body; HoistConstant
    # has to pick it up on a later iteration just like a lowered constant.
    assert not result.exceeded_max_iter, "the two rules must not fight each other"
    assert not any(isinstance(stmt, py.Constant) for stmt in body_stmts(mt))

    folded = threes(outer_block(mt).stmts)
    assert len(folded) == 1, "the folded constant should be hoisted exactly once"
    assert folded[0].result in body_lambda(mt).captured
    assert mt(4) == before


def test_hoists_constants_from_non_entry_blocks():
    """Constants in branch blocks are hoisted; getfield lands in the entry."""

    @basic_no_opt
    def mt(flag: bool):
        def body(i):
            if flag:
                return i + 7
            return i

        return ilist.map(body, ilist.range(3))

    lam = body_lambda(mt)
    assert len(lam.body.blocks) > 1
    assert any(
        isinstance(stmt, py.Constant)
        for block in lam.body.blocks[1:]
        for stmt in block.stmts
    )

    before_true, before_false = mt(True), mt(False)
    n_captured = len(lam.captured)
    result = apply_hoist(mt)

    assert result.has_done_something
    assert not any(isinstance(stmt, py.Constant) for stmt in all_body_stmts(mt))

    lam = body_lambda(mt)
    assert len(lam.captured) == n_captured + 1
    entry = lam.body.blocks[0]
    fields = [stmt for stmt in entry.stmts if isinstance(stmt, func.GetField)]
    assert fields
    assert all(field.obj is entry.args[0] for field in fields)
    # Non-entry blocks must not recreate the constant or load the capture.
    assert not any(
        isinstance(stmt, (py.Constant, func.GetField))
        for block in lam.body.blocks[1:]
        for stmt in block.stmts
    )
    assert mt(True) == before_true
    assert mt(False) == before_false


def test_hoisting_is_idempotent():
    mt = map_kernel()

    first = apply_hoist(mt)
    second = apply_hoist(mt)

    assert first.has_done_something
    assert not second.has_done_something
    assert not first.exceeded_max_iter
