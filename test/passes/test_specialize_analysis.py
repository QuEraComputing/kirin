import pytest

from kirin.passes import Specialize
from kirin.prelude import basic_no_opt
from kirin.analysis import const
from kirin.dialects import debug

kernel = basic_no_opt.add(debug)


@pytest.mark.parametrize("local_closure", [False, True])
def test_repeated_calls_analyzed_once(monkeypatch, local_closure, capsys):
    monkeypatch.setenv("KIRIN_COMPILE_CACHE_DIR", "FALSE")

    @kernel
    def leaf(x: int):
        debug.info("leaf", x)
        return x + 1

    @kernel
    def level1(x: int):
        return leaf(x) + leaf(x)

    @kernel
    def level2(x: int):
        return level1(x) + level1(x)

    @kernel
    def level3(x: int):
        return level2(x) + level2(x)

    @kernel
    def direct(x: int):
        return level3(x) + level3(x)

    @kernel
    def closure(x: int):
        def inner(offset: int):
            return level3(x) + level3(x) + offset

        return inner(3)

    original = const.Propagate.initialize_frame
    visits = {}
    analyses = []  # Keep analyses alive so their IDs cannot be reused.

    def initialize_frame(self, node, *, has_parent_access=False):
        if node is leaf.code:
            analyses.append(self)
            visits[id(self)] = visits.get(id(self), 0) + 1
        return original(self, node, has_parent_access=has_parent_access)

    monkeypatch.setattr(const.Propagate, "initialize_frame", initialize_frame)
    root = closure if local_closure else direct
    Specialize(root.dialects, no_raise=False)(root)
    assert visits
    assert max(visits.values()) == 1
    assert capsys.readouterr().out == ""
    assert root(2) == 48 + (3 if local_closure else 0)
    assert capsys.readouterr().out.count("INFO:") == 16
