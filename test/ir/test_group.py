from kirin.ir import DialectGroup
from kirin.dialects import cf, func
from kirin.dialects.py import data


def test_union():
    group_a = DialectGroup([data, cf])
    group_b = DialectGroup([data, cf])
    group_c = DialectGroup([data, func])
    group_d = DialectGroup([data, func, cf])

    target_a = group_a.union(group_b)
    target_b = group_a.union(group_c)
    assert target_a.data == group_a.data
    assert target_b.data == group_d.data
