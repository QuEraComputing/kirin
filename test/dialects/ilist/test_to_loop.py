from typing import Any, Literal
from collections.abc import Callable

import pytest

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.passes import TypeInfer
from kirin.prelude import structural_no_opt
from kirin.rewrite import Walk, Chain
from kirin.dialects import py, scf, func, ilist
from kirin.rewrite.abc import RewriteRule
from kirin.dialects.ilist.rewrite.to_loop import (
    AllToForLoop,
    AnyToForLoop,
    MapToForLoop,
    FoldToForLoop,
    ScanToForLoop,
    ForEachToForLoop,
)

Recording = tuple[ir.DialectGroup, Callable[[int], None], list[int]]


@pytest.fixture
def recording() -> Recording:
    dialect = ir.Dialect("test.record")
    recorded: list[int] = []

    @statement(dialect=dialect)
    class Record(ir.Statement):
        traits = frozenset({lowering.FromPythonCall()})
        value: ir.SSAValue = info.argument(types.Int)

    @dialect.register
    class Concrete(interp.MethodTable):
        @interp.impl(Record)
        def record_value(
            self, interpreter: interp.Interpreter, frame: interp.Frame, stmt: Record
        ) -> tuple[()]:
            recorded.append(frame.get_casted(stmt.value, int))
            return ()

    @lowering.wraps(Record)
    def record(value: int) -> None: ...

    return structural_no_opt.add(dialect), record, recorded


def lower(
    method: ir.Method[..., Any],
    rule: RewriteRule | None = None,
    statement_types: tuple[type[ir.Statement], ...] = (ilist.Map,),
) -> tuple[types.TypeAttribute, ...]:
    TypeInfer(method.dialects, no_raise=False)(method)
    original_types = tuple(
        result.type
        for stmt in method.code.walk()
        if isinstance(stmt, statement_types)
        for result in stmt.results
    )
    walk = Walk(MapToForLoop() if rule is None else rule)
    result = walk.rewrite(method.code)
    assert result.has_done_something
    assert not result.exceeded_max_iter
    method.verify()
    assert not any(isinstance(stmt, statement_types) for stmt in method.code.walk())
    repeated = walk.rewrite(method.code)
    assert not repeated.has_done_something
    assert not repeated.exceeded_max_iter
    return original_types


@pytest.mark.parametrize(
    ("values", "expected"),
    [([], []), ([3], [7]), ([1, 2, 3], [3, 5, 7]), ([-2, 0, 4], [-3, 1, 9])],
)
def test_map_to_loop(values: list[int], expected: list[int]) -> None:
    @structural_no_opt
    def transform(x: int) -> int:
        return 2 * x + 1

    @structural_no_opt
    def mapped(xs: ilist.IList[int, Any]) -> ilist.IList[int, Any]:
        return ilist.map(transform, xs)

    xs = ilist.IList(values.copy(), elem=types.Int)
    assert list(mapped(xs)) == expected
    lower(mapped)
    assert list(mapped(xs)) == expected
    assert list(xs) == values
    assert sum(isinstance(stmt, scf.For) for stmt in mapped.code.walk()) == 1


def test_map_preserves_element_and_length_types() -> None:
    @structural_no_opt
    def positive(x: int) -> bool:
        return x > 0

    @structural_no_opt
    def mapped(xs: ilist.IList[int, Literal[3]]) -> ilist.IList[bool, Literal[3]]:
        return ilist.map(positive, xs)

    xs = ilist.IList([-1, 0, 1], elem=types.Int)
    assert list(mapped(xs)) == [False, False, True]
    lower(mapped)
    assert list(mapped(xs)) == [False, False, True]
    loop = next(stmt for stmt in mapped.code.walk() if isinstance(stmt, scf.For))
    assert loop.results[0].type == ilist.IListType[types.Bool, types.Literal(3)]
    assert loop.body.blocks[0].args[1].type == ilist.IListType[types.Bool, types.Any]
    call = next(stmt for stmt in loop.body.walk() if isinstance(stmt, func.Call))
    assert call.result.type == types.Bool
    empty = loop.initializers[0]
    assert empty.type == ilist.IListType[types.Bool, types.Literal(0)]


def test_map_captured_value() -> None:
    @structural_no_opt
    def mapped(xs: ilist.IList[int, Any], offset: int) -> ilist.IList[int, Any]:
        def shift(x: int) -> int:
            return x + offset

        return ilist.map(shift, xs)

    xs = ilist.IList([1, 4], elem=types.Int)
    assert list(mapped(xs, 7)) == [8, 11]
    lower(mapped)
    assert list(mapped(xs, 7)) == [8, 11]


@pytest.mark.parametrize("values", [[], [3, 1, 3]])
def test_map_callback_effects(values: list[int], recording: Recording) -> None:
    dialects, record, recorded = recording

    @dialects
    def emit(x: int) -> int:
        record(x)
        return x + 1

    @dialects
    def mapped(xs: ilist.IList[int, Any]) -> ilist.IList[int, Any]:
        return ilist.map(emit, xs)

    xs = ilist.IList(values, elem=types.Int)
    assert list(mapped(xs)) == [x + 1 for x in values]
    assert recorded == values
    recorded.clear()
    lower(mapped)
    assert list(mapped(xs)) == [x + 1 for x in values]
    assert recorded == values
    recorded.clear()


@pytest.mark.parametrize(
    ("values", "initial", "expected"),
    [([], 7, 7), ([3], 2, 23), ([1, 2, 3], 0, 123), ([1, 2, 3], 4, 4123)],
)
def test_foldl_to_loop(values: list[int], initial: int, expected: int) -> None:
    @structural_no_opt
    def step(acc: int, x: int) -> int:
        return 10 * acc + x

    @structural_no_opt
    def folded(xs: ilist.IList[int, Any], initial: int) -> int:
        return ilist.foldl(step, xs, initial)

    xs = ilist.IList(values.copy(), elem=types.Int)
    assert folded(xs, initial) == expected
    lower(folded, FoldToForLoop(), (ilist.Foldl, ilist.Foldr))
    assert folded(xs, initial) == expected
    assert list(xs) == values
    loops = [stmt for stmt in folded.code.walk() if isinstance(stmt, scf.For)]
    assert len(loops) == 1
    assert loops[0].results[0].type == types.Int
    assert loops[0].body.blocks[0].args[1].type == types.Int


def test_foldl_distinct_accumulator_type() -> None:
    @structural_no_opt
    def step(acc: tuple[int, int], x: int) -> tuple[int, int]:
        return (acc[0] + x, acc[1] + 1)

    @structural_no_opt
    def folded(xs: ilist.IList[int, Any]) -> tuple[int, int]:
        return ilist.foldl(step, xs, (10, 0))

    xs = ilist.IList([2, 3, 4], elem=types.Int)
    assert folded(xs) == (19, 3)
    lower(folded, FoldToForLoop(), (ilist.Foldl, ilist.Foldr))
    assert folded(xs) == (19, 3)
    loop = next(stmt for stmt in folded.code.walk() if isinstance(stmt, scf.For))
    assert loop.results[0].type == types.Tuple[types.Int, types.Int]
    call = next(stmt for stmt in loop.body.walk() if isinstance(stmt, func.Call))
    assert call.inputs[0].type == types.Tuple[types.Int, types.Int]
    assert call.inputs[1].type == types.Int


@pytest.mark.parametrize("values", [[], [3, 1, 3]])
def test_foldl_callback_effects(values: list[int], recording: Recording) -> None:
    dialects, record, recorded = recording

    @dialects
    def step(acc: int, x: int) -> int:
        record(x)
        return acc + x

    @dialects
    def folded(xs: ilist.IList[int, Any]) -> int:
        return ilist.foldl(step, xs, 7)

    xs = ilist.IList(values, elem=types.Int)
    for rewritten in (False, True):
        if rewritten:
            lower(folded, FoldToForLoop(), (ilist.Foldl, ilist.Foldr))
        assert folded(xs) == 7 + sum(values)
        assert recorded == values
        recorded.clear()


@pytest.mark.parametrize(
    ("values", "initial", "expected"),
    [([], 7, 7), ([3], 2, 1), ([1, 2, 3], 0, 2), ([1, 2, 3], 4, -2)],
)
def test_foldr_to_loop(values: list[int], initial: int, expected: int) -> None:
    @structural_no_opt
    def step(x: int, acc: int) -> int:
        return x - acc

    @structural_no_opt
    def folded(xs: ilist.IList[int, Any], initial: int) -> int:
        return ilist.foldr(step, xs, initial)

    xs = ilist.IList(values.copy(), elem=types.Int)
    assert folded(xs, initial) == expected
    lower(folded, FoldToForLoop(), (ilist.Foldl, ilist.Foldr))
    assert folded(xs, initial) == expected
    assert list(xs) == values
    loop = next(stmt for stmt in folded.code.walk() if isinstance(stmt, scf.For))
    assert loop.results[0].type == types.Int


def test_foldr_callback_order(recording: Recording) -> None:
    dialects, record, recorded = recording

    @dialects
    def step(x: int, acc: int) -> int:
        record(x)
        return acc + x

    @dialects
    def folded(xs: ilist.IList[int, Any]) -> int:
        return ilist.foldr(step, xs, 0)

    xs = ilist.IList([1, 2, 3], elem=types.Int)
    for rewritten in (False, True):
        if rewritten:
            lower(folded, FoldToForLoop(), (ilist.Foldl, ilist.Foldr))
        assert folded(xs) == 6
        assert recorded == [3, 2, 1]
        recorded.clear()


@pytest.mark.parametrize(
    ("values", "initial", "expected_state", "expected_outputs"),
    [([], 7, 7, []), ([3], 2, 23, [23]), ([1, 2, 3], 0, 123, [1, 12, 123])],
)
def test_scan_to_loop(
    values: list[int], initial: int, expected_state: int, expected_outputs: list[int]
) -> None:
    @structural_no_opt
    def step(acc: int, x: int) -> tuple[int, int]:
        next_acc = 10 * acc + x
        return next_acc, next_acc

    @structural_no_opt
    def scanned(
        xs: ilist.IList[int, Any], initial: int
    ) -> tuple[int, ilist.IList[int, Any]]:
        return ilist.scan(step, xs, initial)

    xs = ilist.IList(values.copy(), elem=types.Int)
    for rewritten in (False, True):
        if rewritten:
            lower(scanned, ScanToForLoop(), (ilist.Scan,))
        state, outputs = scanned(xs, initial)
        assert state == expected_state
        assert list(outputs) == expected_outputs
        assert list(xs) == values


def test_scan_distinct_output_type_and_length() -> None:
    @structural_no_opt
    def step(acc: int, x: int) -> tuple[int, bool]:
        next_acc = acc + x
        return next_acc, next_acc > 3

    @structural_no_opt
    def scanned(
        xs: ilist.IList[int, Literal[3]],
    ) -> tuple[int, ilist.IList[bool, Literal[3]]]:
        return ilist.scan(step, xs, 0)

    xs = ilist.IList([1, 2, 3], elem=types.Int)
    for rewritten in (False, True):
        if rewritten:
            lower(scanned, ScanToForLoop(), (ilist.Scan,))
        state, outputs = scanned(xs)
        assert state == 6
        assert list(outputs) == [False, False, True]
    loop = next(stmt for stmt in scanned.code.walk() if isinstance(stmt, scf.For))
    assert loop.results[0].type == types.Int
    assert loop.results[1].type == ilist.IListType[types.Bool, types.Literal(3)]
    assert loop.body.blocks[0].args[2].type == ilist.IListType[types.Bool, types.Any]


@pytest.mark.parametrize("values", [[], [3, 1, 3]])
def test_scan_callback_effects(values: list[int], recording: Recording) -> None:
    dialects, record, recorded = recording

    @dialects
    def step(acc: int, x: int) -> tuple[int, int]:
        record(x)
        return acc + x, x

    @dialects
    def scanned(xs: ilist.IList[int, Any]) -> tuple[int, ilist.IList[int, Any]]:
        return ilist.scan(step, xs, 7)

    xs = ilist.IList(values, elem=types.Int)
    for rewritten in (False, True):
        if rewritten:
            lower(scanned, ScanToForLoop(), (ilist.Scan,))
        state, outputs = scanned(xs)
        assert state == 7 + sum(values)
        assert list(outputs) == values
        assert recorded == values
        recorded.clear()


@pytest.mark.parametrize("values", [[], [2], [3, 1, 3]])
@pytest.mark.parametrize("enabled", [False, True])
def test_for_each_effects_and_captures(
    values: list[int], enabled: bool, recording: Recording
) -> None:
    dialects, record, recorded = recording

    @dialects
    def visit(xs: ilist.IList[int, Any], offset: int, enabled: bool) -> int:
        def emit(x: int) -> None:
            record(x + offset)

        if enabled:
            ilist.for_each(emit, xs)
        return len(xs)

    xs = ilist.IList(values.copy(), elem=types.Int)
    for rewritten in (False, True):
        if rewritten:
            lower(visit, ForEachToForLoop(), (ilist.ForEach,))
            loop = next(stmt for stmt in visit.code.walk() if isinstance(stmt, scf.For))
            assert len(loop.results) == 0
        assert visit(xs, 5, enabled) == len(values)
        assert recorded == ([x + 5 for x in values] if enabled else [])
        recorded.clear()
        assert list(xs) == values


@pytest.mark.parametrize(
    ("values", "expected"),
    [
        ([], (False, True)),
        ([True], (True, True)),
        ([False], (False, False)),
        ([True, True], (True, True)),
        ([False, False], (False, False)),
        ([False, True, False], (True, False)),
        ([True, False, True], (True, False)),
    ],
)
def test_boolean_reductions(values: list[bool], expected: tuple[bool, bool]) -> None:
    @structural_no_opt
    def reduce(xs: ilist.IList[bool, Any]) -> tuple[bool, bool]:
        return ilist.any(xs), ilist.all(xs)

    xs = ilist.IList(values, elem=types.Bool)
    assert reduce(xs) == expected
    lower(
        reduce,
        Chain(AnyToForLoop(), AllToForLoop()),
        (ilist.stmts.Any, ilist.stmts.All),
    )
    assert reduce(xs) == expected
    loops = [stmt for stmt in reduce.code.walk() if isinstance(stmt, scf.For)]
    assert len(loops) == 2
    assert all(loop.results[0].type == types.Bool for loop in loops)


def test_iteration_rules_non_match() -> None:
    for rule in (
        MapToForLoop(),
        FoldToForLoop(),
        ScanToForLoop(),
        ForEachToForLoop(),
        AnyToForLoop(),
        AllToForLoop(),
    ):
        assert not rule.rewrite(py.Constant(1)).has_done_something


def test_pass_rewrites_every_higher_order_statement() -> None:
    @structural_no_opt
    def double(x: int) -> int:
        return 2 * x

    @structural_no_opt
    def add(acc: int, x: int) -> int:
        return acc + x

    @structural_no_opt
    def running(acc: int, x: int) -> tuple[int, int]:
        return acc + x, acc + x

    @structural_no_opt
    def positive(x: int) -> bool:
        return x > 0

    @structural_no_opt
    def kernel(xs: ilist.IList[int, Any]) -> tuple[int, int, int, bool, bool]:
        doubled = ilist.map(double, xs)
        ilist.for_each(double, doubled)
        signs = ilist.map(positive, doubled)
        scanned = ilist.scan(running, doubled, 0)
        return (
            ilist.foldl(add, doubled, 0),
            ilist.foldr(add, doubled, 0),
            ilist.foldl(add, scanned[1], 0),
            ilist.any(signs),
            ilist.all(signs),
        )

    xs = ilist.IList([1, 2, 3], elem=types.Int)
    expected = kernel(xs)
    assert expected == (12, 12, 20, True, True)

    pass_ = ilist.IListToLoop(kernel.dialects, no_raise=False)
    assert not kernel.inferred
    assert pass_(kernel).has_done_something
    assert kernel.inferred
    assert not any(
        isinstance(
            stmt,
            (
                ilist.Map,
                ilist.Foldl,
                ilist.Foldr,
                ilist.Scan,
                ilist.ForEach,
                ilist.stmts.Any,
                ilist.stmts.All,
            ),
        )
        for stmt in kernel.code.walk()
    )
    assert sum(isinstance(stmt, scf.For) for stmt in kernel.code.walk()) == 9
    assert kernel(xs) == expected
    assert list(xs) == [1, 2, 3]
    assert not pass_(kernel).has_done_something


@pytest.mark.parametrize("enabled", [False, True])
@pytest.mark.parametrize("count", [0, 2])
def test_pass_rewrites_nested_statements(enabled: bool, count: int) -> None:
    @structural_no_opt
    def double(x: int) -> int:
        return 2 * x

    @structural_no_opt
    def add(acc: int, x: int) -> int:
        return acc + x

    @structural_no_opt
    def kernel(xs: ilist.IList[int, Any], enabled: bool, count: int) -> int:
        total = 0
        for _ in range(count):
            if enabled:
                total = total + ilist.foldl(add, ilist.map(double, xs), 0)
        return total

    xs = ilist.IList([1, 2, 3], elem=types.Int)
    expected = kernel(xs, enabled, count)
    assert expected == (12 * count if enabled else 0)
    assert ilist.IListToLoop(kernel.dialects, no_raise=False)(kernel).has_done_something
    assert not any(
        isinstance(stmt, (ilist.Map, ilist.Foldl)) for stmt in kernel.code.walk()
    )
    assert kernel(xs, enabled, count) == expected


def test_pass_leaves_unrelated_methods_unchanged() -> None:
    @structural_no_opt
    def kernel(xs: ilist.IList[int, Any]) -> int:
        return len(xs)

    TypeInfer(kernel.dialects, no_raise=False)(kernel)
    assert not ilist.IListToLoop(kernel.dialects, no_raise=False)(
        kernel
    ).has_done_something
    assert not any(isinstance(stmt, scf.For) for stmt in kernel.code.walk())


def test_foldr_distinct_accumulator_type() -> None:
    @structural_no_opt
    def step(x: int, acc: tuple[int, int]) -> tuple[int, int]:
        return (10 * acc[0] + x, acc[1] + 1)

    @structural_no_opt
    def folded(xs: ilist.IList[int, Any]) -> tuple[int, int]:
        return ilist.foldr(step, xs, (0, 0))

    xs = ilist.IList([1, 2, 3], elem=types.Int)
    assert folded(xs) == (321, 3)
    (result_type,) = lower(folded, FoldToForLoop(), (ilist.Foldr,))
    assert result_type == types.Tuple[types.Int, types.Int]
    folded.verify_type()
    assert folded(xs) == (321, 3)
