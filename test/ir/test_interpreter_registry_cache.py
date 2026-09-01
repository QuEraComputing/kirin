"""`Registry.interpreter` caches its table on the dialect group."""

from kirin import ir
from kirin.decl import info, statement
from kirin.interp import Signature, MethodTable, impl
from kirin.prelude import basic_no_opt
from kirin.dialects import py

dialect = ir.Dialect("test_registry_cache")


@statement(dialect=dialect)
class Noop(ir.Statement):
    name = "noop"
    value: int = info.attribute()


def test_same_keys_reuse_the_same_table():
    group = basic_no_opt
    first = group.registry.interpreter(keys=["main"])
    second = group.registry.interpreter(keys=["main"])
    # identity, not just equality: the table is shared, never copied
    assert first is second


def test_different_keys_get_different_tables():
    group = basic_no_opt
    main = group.registry.interpreter(keys=["main"])
    constprop = group.registry.interpreter(keys=["constprop", "main"])
    assert main is not constprop


def test_cache_is_per_group():
    a = ir.DialectGroup([py.constant, py.binop])
    b = ir.DialectGroup([py.constant])
    assert a.registry.interpreter(keys=["main"]) is not b.registry.interpreter(
        keys=["main"]
    )


def test_cached_table_matches_a_fresh_build():
    group = basic_no_opt
    cached = group.registry.interpreter(keys=["main"])
    fresh = group.registry._build_interpreter(("main",))
    assert cached == fresh


def test_registering_a_new_method_table_invalidates_the_cache():
    """`Dialect.register` can add interpreters after a group is built."""
    group = ir.DialectGroup([dialect, py.constant])
    before = group.registry.interpreter(keys=["main"])
    assert Signature(Noop) not in before

    @dialect.register
    class NoopMethods(MethodTable):
        @impl(Noop)
        def noop(self, interp, frame, stmt):
            return ()

    after = group.registry.interpreter(keys=["main"])
    assert after is not before
    assert Signature(Noop) in after
