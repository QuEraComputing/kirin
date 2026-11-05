from typing import Any

from pytest import mark

from kirin import types
from kirin.passes import TypeInfer
from kirin.prelude import structural_no_opt
from kirin.rewrite import Walk
from kirin.analysis import TypeInference
from kirin.dialects import scf, ilist


@structural_no_opt
def identity(x: int) -> int:
    return x


@mark.parametrize("n", [0, 1, 3])
def test_unroll_map_in_growing_loop(n):
    @structural_no_opt
    def grow(n: int) -> ilist.IList[int, Any]:
        xs = ilist.IList([])
        for _ in range(n):
            xs = xs + [4]
            xs = ilist.map(identity, xs)
        return xs

    expected = list(grow(n))
    TypeInfer(grow.dialects, no_raise=False)(grow)
    Walk(ilist.rewrite.Unroll()).rewrite(grow.code)
    assert list(grow(n)) == expected


def test_growing_list_invariant():
    @structural_no_opt
    def grow(n: int):
        xs = [1]
        for i in range(n):
            xs = xs + [i]
        return xs

    frame, result = TypeInference(grow.dialects).run(grow)
    expected = ilist.IListType[types.Int, types.Any]
    assert result == expected
    loop = next(stmt for stmt in grow.code.walk() if isinstance(stmt, scf.For))
    assert all(frame.get(arg) == expected for arg in loop.body.blocks[0].args[1:])


def test_stable_length_and_multiple_carried_values():
    @structural_no_opt
    def loop(n: int):
        fixed = [1]
        growing = [2]
        for i in range(n):
            fixed = [i]
            growing = growing + fixed
        return fixed, growing

    _, result = TypeInference(loop.dialects).run(loop)
    assert (
        result
        == types.Tuple[
            ilist.IListType[types.Int, types.Literal(1)],
            ilist.IListType[types.Int, types.Any],
        ]
    )


def test_nested_loops_and_repeated_inference():
    @structural_no_opt
    def grow(n: int):
        xs = [1]
        for i in range(n):
            for j in range(n):
                xs = xs + [i + j]
        return xs

    expected = list(grow(3))
    inference = TypeInfer(grow.dialects, no_raise=False)
    inference(grow)
    assert grow.return_type == ilist.IListType[types.Int, types.Any]
    first = [value.type for stmt in grow.code.walk() for value in stmt.results]
    inference(grow)
    assert [value.type for stmt in grow.code.walk() for value in stmt.results] == first
    assert list(grow(3)) == expected


def test_changing_element_type():
    @structural_no_opt
    def grow(n: int):
        xs = [1]
        for _ in range(n):
            xs = xs + [1.5]
        return xs

    _, result = TypeInference(grow.dialects).run(grow)
    assert result == ilist.IListType[types.Int | types.Float, types.Any]


def test_nested_tuple_growth_terminates():
    @structural_no_opt
    def nest(n: int):
        value = ()
        for _ in range(n):
            value = (value,)
        return value

    _, result = TypeInference(nest.dialects).run(nest)
    assert result == types.Any


def test_typed_empty_list():
    from kirin.interp import Frame
    from kirin.dialects.ilist.typeinfer import TypeInfer as IListTypeInfer

    stmt = ilist.New((), elem_type=types.Int)
    result = IListTypeInfer().new(TypeInference(structural_no_opt), Frame(stmt), stmt)
    assert result == (ilist.IListType[types.Int, types.Literal(0)],)


def test_inconsistent_lattice_fails_with_bounded_iterations():
    from contextlib import nullcontext
    from unittest.mock import Mock

    from pytest import raises

    from kirin import interp
    from kirin.analysis import ForwardFrame
    from kirin.dialects.scf.typeinfer import TypeInfer as SCFTypeInfer

    @structural_no_opt
    def grow(n: int):
        xs = [1]
        for i in range(n):
            xs = xs + [i]
        return xs

    loop = next(stmt for stmt in grow.code.walk() if isinstance(stmt, scf.For))
    # Model a broken lattice that rejects containment even after widening to top.
    value = Mock(spec=types.TypeAttribute)
    value.is_subseteq.return_value = False
    value.join.return_value = value
    values = (value,) * len(loop.initializers)
    frame = Mock(spec=ForwardFrame)
    frame.get_values.return_value = values
    interpreter = Mock(spec=TypeInference)
    interpreter.frame_eval.return_value = (types.Int,)
    interpreter.frame_call_region.return_value = values
    interpreter.new_frame.side_effect = lambda *args, **kwargs: nullcontext(
        ForwardFrame(loop)
    )

    with raises(
        interp.InterpreterError, match="scf.For type inference did not converge"
    ):
        SCFTypeInfer().for_loop(interpreter, frame, loop)

    assert interpreter.frame_call_region.call_count <= 32
    frame.set_values.assert_not_called()


@mark.parametrize("n", [0, 1, 2, 5])
def test_branch_dependent_growth_preserves_execution(n):
    @structural_no_opt
    def grow(n: int):
        xs = [1]
        for i in range(n):
            if i % 2 == 0:
                xs = xs + [i]
            else:
                xs = xs + [i, i]
            xs = ilist.map(identity, xs)
        return xs

    expected = list(grow(n))
    TypeInfer(grow.dialects, no_raise=False)(grow)
    maps = [stmt for stmt in grow.code.walk() if isinstance(stmt, ilist.Map)]
    assert len(maps) == 1
    assert ilist.IListType[types.Int, types.Any].is_subseteq(maps[0].collection.type)
    Walk(ilist.rewrite.Unroll()).rewrite(grow.code)
    assert list(grow(n)) == expected


@mark.parametrize("n", [0, 1, 2, 5])
def test_dependent_carried_lists_preserve_execution(n):
    @structural_no_opt
    def grow(n: int):
        xs = [1]
        ys = [2]
        for i in range(n):
            xs = ilist.map(identity, ys)
            ys = xs + [i]
        return xs, ys

    before_xs, before_ys = grow(n)
    TypeInfer(grow.dialects, no_raise=False)(grow)
    mapped = next(stmt for stmt in grow.code.walk() if isinstance(stmt, ilist.Map))
    assert mapped.collection.type == ilist.IListType[types.Int, types.Any]
    Walk(ilist.rewrite.Unroll()).rewrite(grow.code)
    after_xs, after_ys = grow(n)
    assert list(after_xs) == list(before_xs)
    assert list(after_ys) == list(before_ys)


@mark.parametrize("budget", [0, 1, 16])
def test_budget_fallback_preserves_stable_components(monkeypatch, budget):
    monkeypatch.setattr("kirin.dialects.scf.typeinfer._LOOP_WIDENING_BUDGET", budget)

    @structural_no_opt
    def nest(n: int):
        value = ()
        fixed = [1]
        for i in range(n):
            value = (value,)
            fixed = [i]
        return value, fixed

    before_value, before_fixed = nest(3)
    TypeInfer(nest.dialects, no_raise=False)(nest)
    assert (
        nest.return_type
        == types.Tuple[types.Any, ilist.IListType[types.Int, types.Literal(1)]]
    )
    Walk(ilist.rewrite.Unroll()).rewrite(nest.code)
    after_value, after_fixed = nest(3)
    assert after_value == before_value
    assert list(after_fixed) == list(before_fixed)


def test_fallback_propagates_through_dependent_values(monkeypatch):
    monkeypatch.setattr("kirin.dialects.scf.typeinfer._LOOP_WIDENING_BUDGET", 0)

    @structural_no_opt
    def nest(n: int):
        first = ()
        second = ()
        third = ()
        for _ in range(n):
            first = second
            second = third
            third = (third,)
        return first, second, third

    expected = [nest(n) for n in (0, 1, 4)]
    TypeInfer(nest.dialects, no_raise=False)(nest)
    assert nest.return_type == types.Tuple[types.Any, types.Any, types.Any]
    loop = next(stmt for stmt in nest.code.walk() if isinstance(stmt, scf.For))
    assert all(arg.type == types.Any for arg in loop.body.blocks[0].args[1:])
    assert [nest(n) for n in (0, 1, 4)] == expected
