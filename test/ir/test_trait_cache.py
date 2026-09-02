"""`Statement.get_trait` and friends are memoized; check they stay correct."""

import pytest

from kirin import ir
from kirin.ir import Pure, Statement
from kirin.decl import info, statement
from kirin.dialects import py
from kirin.ir.nodes.stmt import _trait_cache

dialect = ir.Dialect("test_trait_cache")


@statement(dialect=dialect)
class WithPure(Statement):
    name = "with_pure"
    traits = frozenset({Pure()})
    value: int = info.attribute()


@statement(dialect=dialect)
class WithoutPure(Statement):
    name = "without_pure"
    value: int = info.attribute()


def test_get_trait_hit_and_miss():
    assert isinstance(WithPure.get_trait(Pure), Pure)
    assert WithoutPure.get_trait(Pure) is None

    # repeated queries go through the cache and must agree
    assert isinstance(WithPure.get_trait(Pure), Pure)
    assert WithoutPure.get_trait(Pure) is None


def test_has_trait_matches_get_trait():
    assert WithPure.has_trait(Pure) is True
    assert WithoutPure.has_trait(Pure) is False


def test_negative_result_is_cached_not_recomputed():
    _trait_cache.pop((WithoutPure, Pure), None)
    assert WithoutPure.get_trait(Pure) is None
    # a cached `None` must be distinguishable from "not cached yet"
    assert (WithoutPure, Pure) in _trait_cache
    assert WithoutPure.get_trait(Pure) is None


def test_sibling_classes_do_not_share_entries():
    """A cache keyed only on the trait would leak between statement classes."""
    assert WithPure.has_trait(Pure) is True
    assert WithoutPure.has_trait(Pure) is False
    assert WithPure.has_trait(Pure) is True


def test_get_present_trait():
    assert isinstance(WithPure.get_present_trait(Pure), Pure)
    with pytest.raises(ValueError):
        WithoutPure.get_present_trait(Pure)


def test_matches_uncached_scan_for_real_dialect_statements():
    """Cached answers must equal a direct scan over `cls.traits`."""

    def uncached(cls, trait):
        for t in cls.traits:
            if isinstance(t, trait):
                return t
        return None

    for cls in (py.Constant, py.Add, py.GetItem, WithPure, WithoutPure):
        for trait in (Pure, ir.ConstantLike, ir.IsTerminator):
            expected = uncached(cls, trait)
            got = cls.get_trait(trait)
            assert (got is None) == (expected is None)
            if expected is not None:
                assert isinstance(got, trait)
            assert cls.has_trait(trait) is (expected is not None)
