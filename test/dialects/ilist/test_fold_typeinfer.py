from typing import Any, Literal

from pytest import mark

from kirin import types
from kirin.passes import Fold, TypeInfer
from kirin.prelude import structural_no_opt
from kirin.rewrite import Walk, Chain, Fixpoint
from kirin.analysis import TypeInference
from kirin.dialects import ilist

# Foldl and Foldr both call `fn(acc, elem)`; they differ in iteration order only.
folds = mark.parametrize("fold", [ilist.foldl, ilist.foldr], ids=["foldl", "foldr"])
hint_consts = mark.parametrize("hint_const", [True, False])


def fold_type(mt, hint_const: bool):
    """Infer the result type of the only fold statement in `mt`.

    With `hint_const`, run the TypeInfer pass (constant hints let the rule infer
    the body of `fn`); otherwise run the bare analysis (only the type of `fn`).
    """
    stmt = next(
        s
        for s in mt.callable_region.walk()
        if isinstance(s, (ilist.Foldl, ilist.Foldr, ilist.Scan))
    )
    if hint_const:
        TypeInfer(mt.dialects, no_raise=False)(mt)
        return stmt.result.type
    frame, _ = TypeInference(mt.dialects).run(mt)
    return frame.get(stmt.result)


def unroll(mt):
    TypeInfer(mt.dialects, no_raise=False)(mt)
    Fixpoint(Walk(Chain(ilist.rewrite.HintLen(), ilist.rewrite.Unroll()))).rewrite(
        mt.code
    )
    Fold(mt.dialects)(mt)


@folds
@hint_consts
def test_fold_seeded_with_empty_list(fold, hint_const):
    @structural_no_opt
    def flatten(xs: ilist.IList[int, Any]):
        def step(acc: ilist.IList[float, Any], i: int):
            return acc + [1.0, 2.0]

        return fold(step, xs, ilist.IList([], elem=types.Float))

    result = fold_type(flatten.similar(), hint_const)
    assert result == ilist.IListType[types.Float, types.Any]


@folds
@hint_consts
def test_fold_closure_seeded_with_empty_list(fold, hint_const):
    @structural_no_opt
    def gather(xs: ilist.IList[int, Any], ys: ilist.IList[float, Any]):
        def step(acc: ilist.IList[float, Any], i: int):
            return acc + [ys[i]]

        return fold(step, xs, ilist.IList([], elem=types.Float))

    result = fold_type(gather.similar(), hint_const)
    assert result == ilist.IListType[types.Float, types.Any]


@folds
@hint_consts
def test_fold_unannotated_step(fold, hint_const):
    @structural_no_opt
    def collect(xs: ilist.IList[int, Any]):
        def step(acc, i):
            return acc + [i]

        return fold(step, xs, ilist.IList([]))

    result = fold_type(collect.similar(), hint_const)
    assert ilist.IListType[types.Any, types.Any].is_subseteq(result)


@folds
def test_fold_stable_accumulator(fold):
    @structural_no_opt
    def total(xs: ilist.IList[int, Any]):
        def add(acc: int, x: int):
            return acc + x

        return fold(add, xs, 0)

    assert fold_type(total.similar(), hint_const=True) == types.Int


@folds
@mark.parametrize("n", [0, 1, 3])
def test_unroll_after_fold_seeded_with_empty_list(fold, n):
    @structural_no_opt
    def flatten_then_copy(xs: ilist.IList[int, Any]):
        def step(acc: ilist.IList[float, Any], i: int):
            return acc + [1.0, 2.0]

        flat = fold(step, xs, ilist.IList([], elem=types.Float))

        def get(i: int):
            return flat[i]

        return ilist.map(get, ilist.range(len(flat)))

    xs = ilist.IList(list(range(n)))
    expected = list(flatten_then_copy(xs))
    unroll(flatten_then_copy)
    assert list(flatten_then_copy(xs)) == expected


@folds
def test_fold_untyped_collection_type_checks(fold):
    @structural_no_opt
    def collect(indices):
        def step(acc, i):
            return acc + [i]

        return fold(step, indices, ilist.IList([]))

    TypeInfer(collect.dialects, no_raise=False)(collect)
    collect.verify_type()


@hint_consts
def test_scan_seeded_with_empty_list(hint_const):
    @structural_no_opt
    def prefixes(xs: ilist.IList[int, Any]):
        def step(acc: ilist.IList[float, Any], i: int):
            return acc + [1.0], i

        return ilist.scan(step, xs, ilist.IList([], elem=types.Float))

    result = fold_type(prefixes.similar(), hint_const)
    assert (
        result
        == types.Tuple[
            ilist.IListType[types.Float, types.Any],
            ilist.IListType[types.Int, types.Any],
        ]
    )


def test_scan_outputs_follow_collection_length():
    @structural_no_opt
    def running(xs: ilist.IList[int, Literal[3]]):
        def step(acc: int, x: int):
            return acc + x, acc

        return ilist.scan(step, xs, 0)

    result = fold_type(running.similar(), hint_const=True)
    assert (
        result == types.Tuple[types.Int, ilist.IListType[types.Int, types.Literal(3)]]
    )


@mark.parametrize("n", [0, 1, 3])
def test_unroll_after_scan_seeded_with_empty_list(n):
    @structural_no_opt
    def scan_then_copy(xs: ilist.IList[int, Any]):
        def step(acc: ilist.IList[float, Any], i: int):
            return acc + [1.0, 2.0], i

        flat = ilist.scan(step, xs, ilist.IList([], elem=types.Float))[0]

        def get(i: int):
            return flat[i]

        return ilist.map(get, ilist.range(len(flat)))

    xs = ilist.IList(list(range(n)))
    expected = list(scan_then_copy(xs))
    unroll(scan_then_copy)
    assert list(scan_then_copy(xs)) == expected


def test_scan_untyped_collection_type_checks():
    @structural_no_opt
    def collect(indices):
        def step(acc, i):
            return acc + [i], i

        return ilist.scan(step, indices, ilist.IList([]))

    TypeInfer(collect.dialects, no_raise=False)(collect)
    collect.verify_type()
