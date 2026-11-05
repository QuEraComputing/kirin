from kirin import types
from kirin.passes import TypeInfer, aggressive
from kirin.prelude import basic
from kirin.rewrite import Walk
from kirin.dialects import py, ilist, random

ys = ilist.IList([10, 20])


@basic(typeinfer=True, fold=True, aggressive=True)
def main():
    def body(i: int):
        random.seed(ys[i])  # impure: keeps the map from folding to a constant

    ilist.map(body, range(2))


MapType = ilist.IListType[types.NoneType, types.Literal(2)]


def stmts_of(mt, kind):
    return [s for s in mt.callable_region.walk() if isinstance(s, kind)]


def test_unroll_is_not_registered_post_inference():
    # Unrolling is an optimization, so type inference must not trigger it.
    assert not any(
        isinstance(rule, ilist.rewrite.Unroll) for rule in ilist.dialect.rules.inference
    )


def test_map_typeinfer_without_unroll():
    mt = main.similar()
    TypeInfer(mt.dialects, no_raise=False)(mt)
    mt.verify_type()

    (map_stmt,) = stmts_of(mt, ilist.Map)
    assert map_stmt.collection.type == ilist.IListType[types.Int, types.Literal(2)]
    assert map_stmt.result.type == MapType
    assert mt.return_type == types.NoneType


def test_explicit_unroll_preserves_map_type():
    mt = main.similar()
    assert Walk(ilist.rewrite.Unroll()).rewrite(mt.code).has_done_something
    TypeInfer(mt.dialects, no_raise=False)(mt)
    mt.verify_type()

    assert not stmts_of(mt, ilist.Map)
    (new_stmt,) = stmts_of(mt, ilist.New)
    assert new_stmt.result.type == MapType


def test_explicit_unroll_then_fold():
    mt = main.similar()
    Walk(ilist.rewrite.Unroll()).rewrite(mt.code)
    aggressive.Fold(mt.dialects).fixpoint(mt)
    mt.verify_type()

    assert not stmts_of(mt, ilist.Map)
    seeds = [s.value.owner for s in stmts_of(mt, random.stmts.Seed)]
    assert all(isinstance(c, py.Constant) for c in seeds)
    assert [c.value.unwrap() for c in seeds] == [10, 20]
