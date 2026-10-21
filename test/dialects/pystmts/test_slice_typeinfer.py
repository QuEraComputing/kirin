from kirin import types
from kirin.prelude import basic_no_opt
from kirin.analysis import TypeInference
from kirin.dialects import py


@basic_no_opt
def slice_to(n: int):
    return slice(0, n)


@basic_no_opt
def slice_from(n: int):
    return slice(n, None)


def slice_stmt(kernel) -> py.slice.Slice:
    return next(
        stmt
        for stmt in kernel.callable_region.walk()
        if isinstance(stmt, py.slice.Slice)
    )


def infer(kernel):
    frame, _ = TypeInference(kernel.dialects).run(kernel)
    return frame


def test_slice_keeps_none_bounds():
    for kernel in (slice_to, slice_from):
        stmt = slice_stmt(kernel)
        frame = infer(kernel)
        for arg in stmt.args:
            assert not frame.get(arg).is_subseteq(types.Bottom)
        assert frame.get(stmt.result).is_subseteq(types.Slice[types.Int])
        assert frame.get(stmt.step).is_subseteq(types.NoneType)


def test_slice_of_only_none_is_bottom():
    @basic_no_opt
    def only_none():
        return slice(None, None, 1)

    stmt = slice_stmt(only_none)
    assert infer(only_none).get(stmt.result).is_subseteq(types.Bottom)
