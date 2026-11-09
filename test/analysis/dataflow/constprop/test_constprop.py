import itertools
from dataclasses import dataclass
from collections.abc import Sequence

import pytest

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt, structural_no_opt
from kirin.rewrite import Walk, Inline
from kirin.analysis import const
from kirin.dialects import py, scf, ilist
from kirin.passes.fold import Fold

nested_dialect = ir.Dialect("nested")


@statement(dialect=nested_dialect, init=False)
class NestedYield(ir.Statement):
    name = "yield"
    traits = frozenset({ir.IsTerminator()})
    values: tuple[ir.SSAValue, ...] = info.argument(types.Any)

    def __init__(self, *values: ir.SSAValue):
        super().__init__(args=values, args_slice={"values": slice(None)})


@statement(dialect=nested_dialect, init=False)
class NestedRegion(ir.Statement):
    name = "region"
    traits = frozenset({ir.Pure(), ir.SSACFG(), lowering.FromPythonCall()})
    body: ir.Region = info.region(multi=False)

    def __init__(self, body: ir.Region):
        terminator = body.blocks[0].last_stmt
        assert isinstance(terminator, NestedYield)
        super().__init__(
            regions=(body,),
            result_types=tuple(value.type for value in terminator.values),
        )


@nested_dialect.register(key="constprop")
class NestedConstProp(interp.MethodTable):
    visits = 0

    @interp.impl(NestedYield)
    def yield_stmt(self, interp_, frame, stmt: NestedYield):
        return interp.YieldValue(frame.get_values(stmt.values))

    @interp.impl(NestedRegion)
    def region(self, interp_, frame, stmt: NestedRegion):
        type(self).visits += 1
        with interp_.new_frame(stmt, has_parent_access=True) as body_frame:
            results = interp_.frame_call_region(body_frame, stmt, stmt.body)
        frame.entries.update(body_frame.entries)
        return results


# Each pair is equal under Python `==`, but a fold that swaps the two
# constants changes the result of a program.
DIFFERENT_CONSTANTS = [
    (1, 1.0),
    (True, 1),
    (0.0, -0.0),
    (0j, complex(0.0, -0.0)),
    ((1,), (1.0,)),
    ([1], [1.0]),
    (((1, 2), 3), ((1, 2.0), 3)),
    (ilist.IList([1]), ilist.IList([1.0])),
    (ilist.IList([], elem=types.Int), ilist.IList([], elem=types.Float)),
    ({1}, {1.0}),
    (frozenset({1}), frozenset({1.0})),
    ({"a": 1}, {"a": 1.0}),
    ({1: "a"}, {1.0: "a"}),
    (slice(1), slice(1.0)),
    (ir.PyAttr(1, types.Any), ir.PyAttr(1.0, types.Any)),
    ({1, 2.0}, {1.0, 2}),
    ({1: 2.0}, {1.0: 2}),
]

# Each pair holds two separate objects that can replace each other.
SAME_CONSTANTS = [
    (1, 1),
    (2.5, 2.5),
    ("ab", "ab"),
    (None, None),
    ((1, (2.0, "x")), (1, (2.0, "x"))),
    (ilist.IList([1, 2], elem=types.Int), ilist.IList([1, 2], elem=types.Int)),
    ({1, 2}, {2, 1}),
    ({"a": (1, 2.0)}, {"a": (1, 2.0)}),
    (slice(1, 5, 2), slice(1, 5, 2)),
    (range(10**12), range(10**12)),
]


@pytest.mark.parametrize("lhs, rhs", DIFFERENT_CONSTANTS)
def test_constants_that_python_calls_equal_stay_apart(lhs, rhs):
    assert lhs == rhs
    assert const.Value(lhs) != const.Value(rhs)
    assert not const.Value(lhs).is_subseteq(const.Value(rhs))
    assert not const.Value(lhs).is_structurally_equal(const.Value(rhs))
    assert const.Value(lhs).join(const.Value(rhs)) == const.Unknown()
    assert const.Value(lhs).meet(const.Value(rhs)) == const.Bottom()


@pytest.mark.parametrize("lhs, rhs", SAME_CONSTANTS)
def test_equal_constants_of_the_same_type_stay_equal(lhs, rhs):
    assert const.Value(lhs) == const.Value(rhs)
    assert const.Value(lhs).is_subseteq(const.Value(rhs))
    assert const.Value(lhs).join(const.Value(rhs)) == const.Value(lhs)


def test_a_nan_constant_equals_only_itself():
    nan = float("nan")
    assert const.Value(nan) == const.Value(nan)
    assert const.Value(nan) != const.Value(float("nan"))


@dataclass(eq=False)
class Point(ir.Data["Point"]):
    """Hold a point whose `unwrap` returns the point itself."""

    x: float

    def __post_init__(self):
        self.type = types.PyClass(Point)

    def unwrap(self):
        return self

    def __eq__(self, other):
        return isinstance(other, Point) and self.x == other.x

    def __hash__(self):
        return hash(self.x)

    def is_structurally_equal(self, other, context=None):
        return self == other

    def print_impl(self, printer):
        printer.plain_print(repr(self))


@dataclass(eq=False)
class Untyped(ir.Data[int]):
    """Hold an int, and never set the `type` field that `ir.Data` declares."""

    value: int

    def unwrap(self):
        return self.value

    def __eq__(self, other):
        return isinstance(other, Untyped) and self.value == other.value

    def __hash__(self):
        return hash(self.value)

    def print_impl(self, printer):
        printer.plain_print(repr(self))


class Tagged(Sequence):
    """Hold a sequence whose `==` also compares a tag outside the items."""

    def __init__(self, items, tag):
        self.items = list(items)
        self.tag = tag

    def __getitem__(self, index):
        return self.items[index]

    def __len__(self):
        return len(self.items)

    def __eq__(self, other):
        return (
            isinstance(other, Tagged)
            and self.items == other.items
            and self.tag == other.tag
        )

    __hash__ = None  # type: ignore


def test_data_whose_unwrap_returns_itself_compares_with_its_own_eq():
    assert const.Value(Point(1.0)) == const.Value(Point(1.0))
    assert const.Value(Point(1.0)) != const.Value(Point(2.0))


def test_data_without_a_type_field_compares_its_unwrapped_value():
    assert const.Value(Untyped(1)) == const.Value(Untyped(1))
    assert const.Value(Untyped(1)) != const.Value(Untyped(2))


def test_a_sequence_with_extra_state_keeps_its_own_eq():
    assert const.Value(Tagged([1], "a")) == const.Value(Tagged([1], "a"))
    assert const.Value(Tagged([1], "a")) != const.Value(Tagged([1], "b"))


@dataclass(eq=False)
class Linked(ir.Data["Linked | None"]):
    """Hold a value whose `unwrap` returns its partner, so `unwrap` cycles."""

    tag: int
    partner: "Linked | None" = None

    def __post_init__(self):
        self.type = types.PyClass(Linked)

    def unwrap(self):
        return self.partner

    def __eq__(self, other):
        return isinstance(other, Linked) and self.tag == other.tag

    def __hash__(self):
        return hash(self.tag)

    def print_impl(self, printer):
        printer.plain_print(repr(self.tag))


def test_data_whose_unwrap_cycles_compares_without_end():
    lhs, rhs = Linked(1), Linked(1)
    lhs.partner, rhs.partner = Linked(2, lhs), Linked(2, rhs)
    assert const.Value(lhs) == const.Value(rhs)


def nested_tuple(depth: int, leaf):
    value = leaf
    for _ in range(depth):
        value = (value,)
    return value


def test_a_deeply_nested_tuple_compares_without_recursion_error():
    assert const.Value(nested_tuple(900, 1)) == const.Value(nested_tuple(900, 1))
    assert const.Value(nested_tuple(900, 1)) != const.Value(nested_tuple(900, 1.0))


class Counted:
    """Hold an int, and count every call of `==` on any `Counted` value."""

    calls = 0

    def __init__(self, value: int):
        self.value = value

    def __hash__(self):
        return hash(self.value)

    def __eq__(self, other):
        type(self).calls += 1
        return isinstance(other, Counted) and self.value == other.value


def test_large_sets_and_dicts_compare_each_element_a_few_times():
    """Check that set and dict comparisons take time linear in the size.

    A search over all pairs of elements would call `==` about 4 million
    times for 2000 elements. A lookup by hash calls it a few times for each
    element.
    """
    size = 2000
    for build in (set, dict.fromkeys):
        lhs = build(Counted(i) for i in range(size))
        rhs = build(Counted(i) for i in range(size))
        Counted.calls = 0
        assert const.Value(lhs) == const.Value(rhs)
        assert Counted.calls <= 5 * size
    assert const.Value({*range(size), 0.5}) != const.Value({*range(size), 0.5 + 0j})


def test_temporary_values_never_share_an_id_during_a_comparison():
    """Check a pair that the comparison reaches after many temporaries die.

    The comparison reads `.real` and `.imag`, and each read builds a new
    float. If the comparison forgot those floats, Python could give a new
    float the same id, and the comparison would skip the differing pair.
    """
    lhs = (complex(0.0, 0.0), *(complex(i, 0.0) for i in range(2000)))
    rhs = (complex(0.0, -0.0), *(complex(i, 0.0) for i in range(2000)))
    assert const.Value(lhs) != const.Value(rhs)


# The pool holds equal constants as separate objects, so that the `is`
# shortcut in the lattice does not decide those pairs.
CONSTANT_POOL = [
    *(lhs for pair in DIFFERENT_CONSTANTS + SAME_CONSTANTS for lhs in pair),
    float("nan"),
    Point(1.0),
    Point(1.0),
    Point(2.0),
    Untyped(1),
    Untyped(1),
    Tagged([1], "a"),
    Tagged([1], "a"),
    Tagged([1], "b"),
    Tagged([1.0], "a"),
]


def test_equal_constants_are_also_equal_under_python_eq():
    """Check that the lattice never merges two constants that `==` keeps apart.

    A merge of two such constants would fold one of them into the place of
    the other. The test checks every ordered pair of the pool. A pair of one
    object with itself skips the `==` check, because `nan == nan` is False.
    """
    for lhs, rhs in itertools.product(CONSTANT_POOL, repeat=2):
        same = const.Value(lhs) == const.Value(rhs)
        assert same == (const.Value(rhs) == const.Value(lhs)), (lhs, rhs)
        if same and lhs is not rhs:
            assert type(lhs) is type(rhs) and lhs == rhs, (lhs, rhs)


class TestLattice:

    def test_meet(self):
        assert const.Unknown().meet(const.Unknown()) == const.Unknown()
        assert const.Unknown().meet(const.Bottom()) == const.Bottom()
        assert const.Unknown().meet(const.Value(1)) == const.Value(1)
        assert const.Unknown().meet(
            const.PartialTuple((const.Value(1), const.Bottom()))
        ) == const.PartialTuple((const.Value(1), const.Bottom()))
        assert const.Bottom().meet(const.Unknown()) == const.Bottom()
        assert const.Bottom().meet(const.Bottom()) == const.Bottom()
        assert const.Bottom().meet(const.Value(1)) == const.Bottom()
        assert (
            const.Bottom().meet(const.PartialTuple((const.Value(1), const.Bottom())))
            == const.Bottom()
        )
        assert const.Value(1).meet(const.Unknown()) == const.Value(1)
        assert const.Value(1).meet(const.Bottom()) == const.Bottom()
        assert const.Value(1).meet(const.Value(1)) == const.Value(1)
        assert (
            const.Value(1).meet(const.PartialTuple((const.Value(1), const.Bottom())))
            == const.Bottom()
        )
        assert const.PartialTuple((const.Value(1), const.Bottom())).meet(
            const.Unknown()
        ) == const.PartialTuple((const.Value(1), const.Bottom()))
        assert (
            const.PartialTuple((const.Value(1), const.Bottom())).meet(const.Bottom())
            == const.Bottom()
        )
        assert (
            const.PartialTuple((const.Value(1), const.Bottom())).meet(const.Value(1))
            == const.Bottom()
        )
        assert const.PartialTuple((const.Value(1), const.Bottom())).meet(
            const.Value((1, 2))
        ) == const.PartialTuple((const.Value(1), const.Bottom()))
        assert const.PartialTuple((const.Value(1), const.Bottom())).meet(
            const.PartialTuple((const.Value(1), const.Bottom()))
        ) == const.PartialTuple((const.Value(1), const.Bottom()))

    def test_join(self):
        assert const.Unknown().join(const.Unknown()) == const.Unknown()
        assert const.Unknown().join(const.Bottom()) == const.Unknown()
        assert const.Unknown().join(const.Value(1)) == const.Unknown()
        assert (
            const.Unknown().join(const.PartialTuple((const.Value(1), const.Bottom())))
            == const.Unknown()
        )
        assert const.Bottom().join(const.Unknown()) == const.Unknown()
        assert const.Bottom().join(const.Bottom()) == const.Bottom()
        assert const.Bottom().join(const.Value(1)) == const.Value(1)
        assert const.Bottom().join(
            const.PartialTuple((const.Value(1), const.Bottom()))
        ) == const.PartialTuple((const.Value(1), const.Bottom()))
        assert const.PartialTuple((const.Value(1), const.Bottom())).join(
            const.Value((1, 2))
        ) == const.PartialTuple((const.Value(1), const.Value(2)))
        assert const.Value(1).join(const.Unknown()) == const.Unknown()
        assert const.Value(1).join(const.Bottom()) == const.Value(1)
        assert const.Value(1).join(const.Value(1)) == const.Value(1)
        assert const.Value(1).join(const.Value(2)) == const.Unknown()
        assert (
            const.Value(1).join(const.PartialTuple((const.Value(1), const.Bottom())))
            == const.Unknown()
        )

    def test_partial_tuples_of_different_lengths_stay_apart(self):
        short = const.PartialTuple((const.Unknown(), const.Value(0)))
        long = const.PartialTuple((const.Unknown(), const.Value(0), const.Value(7)))
        assert not short.is_subseteq(long)
        assert not long.is_subseteq(short)
        assert not short.is_subseteq(const.Value((1, 0, 7)))
        assert not short.is_structurally_equal(long)
        assert short.join(long) == const.Unknown()
        assert long.join(short) == const.Unknown()
        assert short.join(const.Value((1, 0, 7))) == const.Unknown()
        assert short.meet(long) == const.Bottom()
        assert short.meet(const.Value((1, 0, 7))) == const.Bottom()

    def test_is_structurally_equal(self):
        assert const.Unknown().is_structurally_equal(const.Unknown())
        assert not const.Unknown().is_structurally_equal(const.Bottom())
        assert not const.Unknown().is_structurally_equal(const.Value(1))
        assert const.Bottom().is_structurally_equal(const.Bottom())
        assert not const.Bottom().is_structurally_equal(const.Value(1))
        assert const.Value(1).is_structurally_equal(const.Value(1))
        assert not const.Value(1).is_structurally_equal(const.Value(2))
        assert const.PartialTuple(
            (const.Value(1), const.Bottom())
        ).is_structurally_equal(const.PartialTuple((const.Value(1), const.Bottom())))
        assert not const.PartialTuple(
            (const.Value(1), const.Bottom())
        ).is_structurally_equal(const.PartialTuple((const.Value(1), const.Value(2))))

    def test_partial_tuple(self):
        pt1 = const.PartialTuple((const.Value(1), const.Bottom()))
        pt2 = const.PartialTuple((const.Value(1), const.Bottom()))
        assert pt1.is_structurally_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt1
        assert pt1.meet(pt2) == pt1
        pt2 = const.PartialTuple((const.Value(1), const.Value(2)))
        assert not pt1.is_structurally_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == const.PartialTuple((const.Value(1), const.Value(2)))
        assert pt1.meet(pt2) == const.PartialTuple((const.Value(1), const.Bottom()))
        pt2 = const.PartialTuple((const.Value(1), const.Bottom()))
        assert pt1.is_structurally_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt1
        assert pt1.meet(pt2) == pt1
        pt2 = const.PartialTuple((const.Value(1), const.Unknown()))
        assert not pt1.is_structurally_equal(pt2)
        assert pt1.is_subseteq(pt2)
        assert pt1.join(pt2) == pt2
        assert pt1.meet(pt2) == pt1


@basic_no_opt
def foo(x):
    return x + 1


@basic_no_opt
def goo(x):
    return foo(2), foo(x)


@basic_no_opt
def main():
    return goo(3)


@basic_no_opt
def bar(x):
    return goo(x)[0]


@basic_no_opt
def ntuple(len: int):
    if len == 0:
        return ()
    return (0,) + ntuple(len - 1)


@basic_no_opt
def recurse():
    return ntuple(3)


@basic_no_opt
def loop_carried_conditional_counter(condition: bool) -> int:
    counter = 0
    for _ in range(2):
        if condition:
            counter += 1
    return counter


@basic_no_opt
def loop_with_unreachable_dominated_use(condition: bool) -> int:
    counter = 0
    for _ in range(2):
        if condition:
            counter += 1
        if False:
            counter + 10
    return counter


@basic_no_opt
def loop_carried_pair(condition: bool) -> int:
    left = 0
    right = 0
    for _ in range(2):
        if condition:
            left += 1
            right += 1
    return left + right


@structural_no_opt
def increment_in_nested_region(counter: int) -> int:
    if True:
        counter += 1
    return counter


@basic_no_opt
def loop_carried_nested_region_counter(condition: bool) -> int:
    counter = 0
    for _ in range(2):
        if condition:
            counter = increment_in_nested_region(counter)
    return counter


def nested_loop_carried_pair():
    method = loop_carried_pair.similar(basic_no_opt.add(nested_dialect))
    body_adds = next(
        adds
        for block in method.callable_region.blocks
        if len(adds := tuple(stmt for stmt in block.stmts if stmt.name == "add")) == 2
    )
    nested_add = py.binop.Add(body_adds[0].lhs, body_adds[1].lhs)
    nested_region = NestedRegion(
        ir.Region(ir.Block([nested_add, NestedYield(nested_add.result)]))
    )
    nested_region.insert_before(body_adds[0])
    return method, (body_adds[0].lhs, body_adds[1].lhs)


def frontend_nested_loop_carried_counter():
    method = loop_carried_nested_region_counter.similar(structural_no_opt)
    Walk(Inline(lambda _: True)).rewrite(method.code)
    nested_add = next(
        stmt
        for stmt in method.code.walk()
        if isinstance(stmt, py.binop.Add)
        and stmt.parent_region is not method.callable_region
    )
    return method, nested_add


def test_constprop():
    infer = const.Propagate(basic_no_opt)
    frame, ret = infer.run(main)
    assert ret == const.Value((3, 4))
    assert len(frame.entries) == 3

    frame, ret = infer.run(goo)
    assert ret == const.PartialTuple((const.Value(3), const.Unknown()))
    assert len(frame.entries) == 6
    block = goo.callable_region.blocks[0]
    assert frame.entries[block.stmts.at(1).results[0]] == const.Value(3)
    assert frame.entries[block.stmts.at(2).results[0]] == const.Unknown()
    assert frame.entries[block.stmts.at(3).results[0]] == const.PartialTuple(
        (const.Value(3), const.Unknown())
    )

    _, ret = infer.run(bar)
    assert ret == const.Value(3)

    _, ret = infer.run(foo)
    assert ret == const.Unknown()
    _, ret = infer.run(recurse)
    assert ret == const.Value((0, 0, 0))


def test_constprop_revisits_dominated_loop_carried_use():
    method = loop_carried_conditional_counter.similar()

    frame, _ = const.Propagate(method.dialects).run(method)
    add = next(stmt for stmt in method.code.walk() if stmt.name == "add")

    assert frame.entries[add.result] == const.Unknown()


def test_fold_preserves_loop_carried_conditional_counter():
    method = loop_carried_conditional_counter.similar()

    Fold(method.dialects)(method)

    assert method(True) == 2


def test_constprop_revisits_outer_block_for_use_inside_nested_region():
    method, nested_add = frontend_nested_loop_carried_counter()
    nested_block = nested_add.parent_block
    assert nested_block is not None
    nested_region = nested_block.parent_node
    assert nested_region is not None
    region_stmt = nested_region.parent_node
    assert isinstance(region_stmt, scf.IfElse)
    outer_block = region_stmt.parent_block
    assert outer_block is not None
    assert nested_block is not outer_block

    frame, _ = const.Propagate(method.dialects).run(method)

    assert frame.entries[nested_add.result] == const.Unknown()


def test_dependency_generation_deduplicates_pending_visits():
    method, dependencies = nested_loop_carried_pair()
    NestedConstProp.visits = 0

    frame, _ = const.Propagate(method.dialects).run(method)

    assert tuple(frame.entries[value] for value in dependencies) == (
        const.Unknown(),
        const.Unknown(),
    )
    assert NestedConstProp.visits == 2


def test_dependency_change_does_not_reach_dead_block():
    method = loop_with_unreachable_dominated_use.similar()
    dead_add = tuple(stmt for stmt in method.code.walk() if stmt.name == "add")[-1]

    frame, _ = const.Propagate(method.dialects).run(method)

    assert dead_add.result not in frame.entries


@basic_no_opt
def myfunc(x1: int) -> int:
    return x1 * 2


@basic_no_opt
def _for_loop_test_constp(
    cntr: int,
    x: tuple,
    n_range: int,
):
    if cntr < n_range:
        pos = myfunc(cntr)
        x = x + (cntr, pos)
        return _for_loop_test_constp(
            cntr=cntr + 1,
            x=x,
            n_range=n_range,
        )
    else:
        return x


def test_issue_40():
    constprop = const.Propagate(basic_no_opt)
    frame, ret = constprop.run(
        _for_loop_test_constp,
        const.Value(0),
        const.Value(()),
        const.Value(5),
    )
    assert isinstance(ret, const.Value)
    assert ret.data == _for_loop_test_constp(cntr=0, x=(), n_range=5)


dummy_dialect = ir.Dialect("dummy")


@statement(dialect=dummy_dialect)
class DummyStatement(ir.Statement):
    name = "dummy"
    traits = frozenset({lowering.FromPythonCall()})


def test_intraprocedure_side_effect():

    @basic_no_opt.add(dummy_dialect)
    def side_effect_return_none():
        DummyStatement()

    @basic_no_opt.add(dummy_dialect)
    def side_effect_intraprocedure(cond: bool):
        if cond:
            return side_effect_return_none()
        else:
            x = (1, 2, 3)
            return x

    constprop = const.Propagate(basic_no_opt.add(dummy_dialect))
    frame, ret = constprop.run(side_effect_intraprocedure)
    new_tuple = (
        side_effect_intraprocedure.callable_region.blocks[2].stmts.at(3).results[0]
    )
    assert isinstance(ret, const.Unknown)
    assert frame.entries[new_tuple] == const.Value((1, 2, 3))


def test_interprocedure_true_branch():
    @basic_no_opt.add(dummy_dialect)
    def side_effect_maybe_return_none(cond: bool):
        if cond:
            return
        else:
            DummyStatement()
            return

    @basic_no_opt.add(dummy_dialect)
    def side_effect_true_branch_const(cond: bool):
        if cond:
            return side_effect_maybe_return_none(cond)
        else:
            return cond

    constprop = const.Propagate(basic_no_opt.add(dummy_dialect))
    frame, ret = constprop.run(side_effect_true_branch_const)
    assert isinstance(ret, const.Unknown)  # instead of NotPure
    true_branch = side_effect_true_branch_const.callable_region.blocks[1]
    assert frame.entries[true_branch.stmts.at(0).results[0]] == const.Value(None)


def test_non_pure_recursion():
    @basic_no_opt
    def for_loop_append(cntr: int, x: ilist.IList, n_range: int):
        if cntr < n_range:
            for_loop_append(cntr + 1, x + [cntr], n_range)

        return x

    constprop = const.Propagate(basic_no_opt)
    frame, _ = constprop.run(for_loop_append)
    stmt = for_loop_append.callable_region.blocks[1].stmts.at(3)
    assert isinstance(frame.entries[stmt.results[0]], const.Unknown)


def test_closure_prop():
    dialect = ir.Dialect("dummy2")

    @statement(dialect=dialect)
    class DummyStmt2(ir.Statement):
        name = "dummy2"
        traits = frozenset({lowering.FromPythonCall()})
        value: ir.SSAValue = info.argument(types.Int)
        result: ir.ResultValue = info.result(types.Int)

    @basic_no_opt.add(dialect)
    def non_const_closure(x: int, y: int):
        def inner():
            if False:
                return x + y
            else:
                return 2

        return inner

    @basic_no_opt.add(dialect)
    def non_pure(x: int, y: int):
        def inner():
            if False:
                return x + y
            else:
                DummyStmt2(1)  # type: ignore
                return 2

        return inner

    @basic_no_opt.add(dialect)
    def main():
        x = DummyStmt2(1)  # type: ignore
        x = non_const_closure(x, x)  # type: ignore
        return x()

    constprop = const.Propagate(basic_no_opt.add(dialect))
    frame, ret = constprop.run(main)
    main.print(analysis=frame.entries)
    stmt = main.callable_region.blocks[0].stmts.at(3)
    call_result = frame.entries[stmt.results[0]]
    assert isinstance(call_result, const.Value)
    assert call_result.data == 2

    @basic_no_opt.add(dialect)
    def main2():
        x = DummyStmt2(1)  # type: ignore
        x = non_pure(x, x)  # type: ignore
        return x()

    constprop = const.Propagate(basic_no_opt.add(dialect))
    frame, _ = constprop.run(main2)
    main2.print(analysis=frame.entries)
    stmt = main2.callable_region.blocks[0].stmts.at(3)
    call_result = frame.entries[stmt.results[0]]
    assert isinstance(call_result, const.Value)


def test_issue_300():
    @basic_no_opt
    def my_ps(val: float):
        def my_ps_impl():
            return val * 0.3

        return my_ps_impl

    @basic_no_opt
    def my_ps2(val: float):
        my_ps_impl = my_ps(val)
        return my_ps_impl()

    prop = const.Propagate(basic_no_opt)
    frame, ret = prop.run(my_ps2)
    invoke = my_ps2.callable_region.blocks[0].stmts.at(0)
    call = my_ps2.callable_region.blocks[0].stmts.at(1)
    assert invoke in frame.should_be_pure
    assert call in frame.should_be_pure
