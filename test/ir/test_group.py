from kirin.ir import DialectGroup
from kirin.dialects import cf, func
from kirin.dialects.py import base


def test_union():
    group_a = DialectGroup([base, cf])
    group_b = DialectGroup([base, cf])
    group_c = DialectGroup([base, func])
    group_d = DialectGroup([base, func, cf])

    target_a = group_a.union(group_b)
    target_b = group_a.union(group_c)
    assert target_a.data == group_a.data
    assert target_b.data == group_d.data
    assert repr(target_a) == "DialectGroup([base, cf])"
    assert repr(target_b) == "DialectGroup([base, cf, func])"


def test_discard():
    group_a = DialectGroup([base, cf])
    group_c = DialectGroup([base, func])
    group_d = DialectGroup([base, func, cf])

    target_a = group_d.discard(cf)
    target_b = group_d.discard(func)
    assert target_a.data == group_c.data
    assert target_b.data == group_a.data
