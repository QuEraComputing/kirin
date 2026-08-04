from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt
from kirin.analysis import const
from kirin.dialects import py, ilist
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


def nested_loop_carried_conditional_counter():
    method = loop_carried_conditional_counter.similar(basic_no_opt.add(nested_dialect))
    add = next(stmt for stmt in method.code.walk() if stmt.name == "add")
    nested_add = py.binop.Add(add.lhs, add.rhs)
    nested_region = NestedRegion(
        ir.Region(ir.Block([nested_add, NestedYield(nested_add.result)]))
    )
    nested_region.insert_before(add)
    add.result.replace_by(nested_region.results[0])
    add.delete()
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


def test_constprop_revisits_use_inside_nested_region():
    method, nested_add = nested_loop_carried_conditional_counter()

    frame, _ = const.Propagate(method.dialects).run(method)

    assert frame.entries[nested_add.result] == const.Unknown()


def test_dependency_generation_deduplicates_pending_visits():
    method, _ = nested_loop_carried_conditional_counter()
    NestedConstProp.visits = 0

    const.Propagate(method.dialects).run(method)

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
