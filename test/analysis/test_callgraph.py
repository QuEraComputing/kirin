from kirin.prelude import basic_no_opt
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
    assert "cde" in graph.get_neighbors("abc")
    assert "defg" in graph.get_neighbors("abc")
    assert "cde" in graph.get_neighbors("abc")
    assert "defg" in graph.get_neighbors("abc")
