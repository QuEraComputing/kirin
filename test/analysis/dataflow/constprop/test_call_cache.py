from kirin.prelude import basic_no_opt
from kirin.analysis import const
from kirin.analysis.const import prop


def _count_analyses(monkeypatch) -> dict[str, int]:
    """Count calls the cache misses, i.e. callee bodies actually analyzed."""
    analyses: dict[str, int] = {}
    call = prop.ForwardExtra.call

    def counting_call(self, node, *args, **kwargs):
        name = getattr(node, "sym_name", None)
        analyses[name] = analyses.get(name, 0) + 1
        return call(self, node, *args, **kwargs)

    monkeypatch.setattr(prop.ForwardExtra, "call", counting_call)
    return analyses


@basic_no_opt
def add_one(x: float):
    return x + 1.0


@basic_no_opt
def add_one_twice(a: float, b: float):
    return add_one(a) + add_one(b)


def test_equal_constants_in_different_objects_share_one_analysis(monkeypatch):
    analyses = _count_analyses(monkeypatch)
    a, b = const.Value(float("2.5")), const.Value(float("2.5"))
    assert a.data is not b.data

    _, result = const.Propagate(basic_no_opt, cache_calls=True).run(add_one_twice, a, b)

    assert analyses["add_one"] == 1
    assert result == const.Value(7.0)


def test_signed_zeros_do_not_share_an_analysis(monkeypatch):
    analyses = _count_analyses(monkeypatch)

    const.Propagate(basic_no_opt, cache_calls=True).run(
        add_one_twice, const.Value(0.0), const.Value(-0.0)
    )

    assert analyses["add_one"] == 2


@basic_no_opt
def scale_by(k: float):
    def scaled(y: float):
        return y * k

    return scaled


@basic_no_opt
def apply(f, x: float):
    return f(x)


@basic_no_opt
def apply_two_equal_closures(k: float):
    return apply(scale_by(k), 3.0) + apply(scale_by(k), 3.0)


def test_equal_closures_share_one_analysis(monkeypatch):
    uncached, expected = const.Propagate(basic_no_opt).run(
        apply_two_equal_closures, const.Value(2.0)
    )
    analyses = _count_analyses(monkeypatch)

    _, result = const.Propagate(basic_no_opt, cache_calls=True).run(
        apply_two_equal_closures, const.Value(2.0)
    )

    # Each scale_by call builds a new closure object, equal in code and
    # captures; the second apply reuses the first one's analysis.
    assert analyses["apply"] == 1
    assert result == expected == const.Value(12.0)
