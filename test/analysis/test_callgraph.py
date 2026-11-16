from kirin.prelude import basic_no_opt
from kirin.dialects import func
from kirin.analysis.callgraph import CallGraph


@basic_no_opt
def abc(a, b):
    return a + b


@basic_no_opt
def bcd(a, b):
    return a - b


@basic_no_opt
def cde(a, b):
    return abc(a, b) + bcd(a, b)


@basic_no_opt
def defg(a, b):
    return cde(a, b) + abc(a, b)


@basic_no_opt
def efg(a, b):
    return defg(a, b) + bcd(a, b)


def test_callgraph():
    graph = CallGraph(efg)
    graph.print()
    assert cde in graph.get_neighbors(abc)
    assert defg in graph.get_neighbors(abc)
    assert cde in graph.get_neighbors(abc)
    assert defg in graph.get_neighbors(abc)


def test_callgraph_direct_recursion():
    @basic_no_opt
    def recursive(n: int) -> int:
        if n > 0:
            return recursive(n - 1)
        return 0

    call = next(
        stmt for stmt in recursive.callable_region.walk() if isinstance(stmt, func.Call)
    )
    call.replace_by(func.Invoke(call.args[1:], callee=recursive))

    graph = CallGraph(recursive)
    assert set(graph.get_neighbors(recursive)) == {recursive}
    assert set(graph.get_edges()) == {(recursive, recursive)}
    assert set(graph.get_nodes()) == {recursive}
    assert graph.backedges == {recursive: {recursive}}


def test_callgraph_mutual_recursion():
    @basic_no_opt
    def pong(n: int) -> int:
        return n

    @basic_no_opt
    def ping(n: int) -> int:
        return pong(n)

    ret = next(
        stmt for stmt in pong.callable_region.walk() if isinstance(stmt, func.Return)
    )
    call = func.Invoke((ret.args[0],), callee=ping)
    call.insert_before(ret)
    ret.replace_by(func.Return(call.result))

    graph = CallGraph(ping)
    assert set(graph.get_edges()) == {(pong, ping), (ping, pong)}
    assert set(graph.get_nodes()) == {ping, pong}
    assert graph.backedges == {ping: {pong}, pong: {ping}}


def test_callgraph_shared_callee(monkeypatch):
    @basic_no_opt
    def left(a, b):
        return abc(a, b)

    @basic_no_opt
    def right(a, b):
        return abc(a, b)

    @basic_no_opt
    def root(a, b):
        return left(a, b) + right(a, b)

    region_type = type(abc.callable_region)
    walk = region_type.walk
    visited_regions = []

    def counted_walk(region, *args, **kwargs):
        visited_regions.append(region)
        return walk(region, *args, **kwargs)

    monkeypatch.setattr(region_type, "walk", counted_walk)
    graph = CallGraph(root)
    assert set(graph.get_neighbors(abc)) == {left, right}
    assert set(graph.get_edges()) == {
        (left, root),
        (right, root),
        (abc, left),
        (abc, right),
    }
    for method in (root, left, right, abc):
        assert sum(region is method.callable_region for region in visited_regions) == 1


def test_callgraph_acyclic():
    graph = CallGraph(efg)
    assert set(graph.get_edges()) == {
        (defg, efg),
        (bcd, efg),
        (cde, defg),
        (abc, defg),
        (abc, cde),
        (bcd, cde),
    }
    assert set(graph.get_nodes()) == {defg, cde, abc, bcd}
    assert set(graph.get_neighbors(efg)) == set()


def test_callgraph_reverse_edges():
    graph = CallGraph(efg)
    assert graph.backedges == {
        efg: {defg, bcd},
        defg: {cde, abc},
        cde: {abc, bcd},
    }
