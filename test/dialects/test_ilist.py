from typing import Any, Literal

from kirin import ir, types, rewrite
from kirin.decl import info, statement
from kirin.passes import aggressive
from kirin.prelude import structural, basic_no_opt, python_basic
from kirin.analysis import const
from kirin.dialects import py, func, ilist, lowering
from kirin.lowering import FromPythonCall
from kirin.passes.typeinfer import TypeInfer


@ir.dialect_group(
    python_basic.union([func, ilist, lowering.func, lowering.range.ilist])
)
def basic(self):
    aggressive_fold_pass = aggressive.Fold(self)
    typeinfer_pass = TypeInfer(self)

    def run_pass(
        mt: ir.Method,
    ) -> None:
        aggressive_fold_pass.fixpoint(mt)
        rewrite.Fixpoint(rewrite.Walk(ilist.rewrite.ConstList2IList())).rewrite(mt.code)
        typeinfer_pass(mt)

    return run_pass


def test_empty():
    @basic
    def empty_list():
        return []

    empty_list.print()
    assert empty_list.return_type.is_subseteq(ilist.IListType[types.Any])


def test_typehint():
    @basic
    def main(xs: ilist.IList[int, Literal[3]]):
        return xs + [4, 5, 6] + xs

    assert main.return_type is not None
    assert main.return_type.is_subseteq(ilist.IListType[types.Int, types.Literal(9)])


@basic
def add1(x: int):
    return x + 1


def test_ilist_fcf():
    # TODO: actually check equivalent code
    rule = rewrite.Fixpoint(rewrite.Walk(ilist.rewrite.Unroll()))

    xs = ilist.IList([1, 2, 3])

    @basic
    def map(xs: ilist.IList[int, Literal[3]]):
        return ilist.map(add1, xs)

    @basic_no_opt
    def foreach(xs: ilist.IList[int, Literal[3]]):
        ilist.for_each(add1, xs)

    map_before = map(xs)
    foreach_before = foreach(xs)
    rule.rewrite(map.code)
    rule.rewrite(foreach.code)
    map_after = map(xs)
    foreach_after = foreach(xs)
    assert map_before.data == map_after.data  # type: ignore
    assert foreach_before == foreach_after

    assert isinstance(map.callable_region.blocks[0].stmts.at(1), py.Constant)
    assert isinstance(map.callable_region.blocks[0].stmts.at(-2), ilist.New)

    assert isinstance(foreach.callable_region.blocks[0].stmts.at(1), py.Constant)
    assert isinstance(
        foreach.callable_region.blocks[0].stmts.at(2), py.indexing.GetItem
    )
    assert isinstance(foreach.callable_region.blocks[0].stmts.at(3), func.Call)
    assert isinstance(foreach.callable_region.blocks[0].stmts.at(10), func.ConstantNone)

    @basic
    def add(x: int, y: int):
        return x + y, y

    @basic
    def scan(xs: ilist.IList[int, Literal[3]]):
        return ilist.Scan(add, xs, init=123)  # type: ignore

    scan_before = scan(xs)
    rule.rewrite(scan.code)
    scan_after = scan(xs)
    assert scan_before == scan_after  # type: ignore
    assert isinstance(scan.callable_region.blocks[0].stmts.at(-2), py.tuple.New)
    assert isinstance(scan.callable_region.blocks[0].stmts.at(-3), ilist.New)

    @basic
    def add2(x: int, y: int):
        return x + y

    @basic
    def foldl(xs: ilist.IList[int, Literal[3]]):
        return ilist.Foldl(add2, xs, init=123)  # type: ignore

    @basic
    def foldr(xs: ilist.IList[int, Literal[3]]):
        return ilist.Foldr(add2, xs, init=123)  # type: ignore

    foldl_before = foldl(xs)
    foldr_before = foldr(xs)
    rule.rewrite(foldl.code)
    rule.rewrite(foldr.code)
    foldl_after = foldl(xs)
    foldr_after = foldr(xs)

    assert foldl_before == foldl_after  # type: ignore
    assert foldr_before == foldr_after  # type: ignore

    stmt = foldl.callable_region.blocks[0].stmts.at(2)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 0
    assert isinstance(foldl.callable_region.blocks[0].stmts.at(4), func.Call)

    stmt = foldl.callable_region.blocks[0].stmts.at(5)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 1
    assert isinstance(foldl.callable_region.blocks[0].stmts.at(7), func.Call)

    stmt = foldl.callable_region.blocks[0].stmts.at(8)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 2
    assert isinstance(foldl.callable_region.blocks[0].stmts.at(10), func.Call)

    # ========== foldl
    stmt = foldr.callable_region.blocks[0].stmts.at(2)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 2
    assert isinstance(foldr.callable_region.blocks[0].stmts.at(4), func.Call)

    stmt = foldr.callable_region.blocks[0].stmts.at(5)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 1
    assert isinstance(foldr.callable_region.blocks[0].stmts.at(7), func.Call)

    stmt = foldr.callable_region.blocks[0].stmts.at(8)
    assert isinstance(stmt, py.Constant)
    assert stmt.value.unwrap() == 0
    assert isinstance(foldr.callable_region.blocks[0].stmts.at(10), func.Call)


def test_ilist_range():
    @basic.add(py.range)
    def map():
        return ilist.Map(add1, range(0, 3))  # type: ignore

    assert map() == ilist.IList([1, 2, 3])

    @basic.add(py.range)
    def const_range():
        return range(0, 3)

    assert const_range() == ilist.IList(range(0, 3))


def test_inline_get_item():
    items = tuple(ir.TestValue() for _ in range(2))

    test_block = ir.Block(
        [
            qreg := ilist.New(values=items),
            idx := py.Constant(0),
            qubit_stmt := py.GetItem(obj=qreg.result, index=idx.result),
            ilist.New(values=(qubit_stmt.result,)),
            idx1 := py.Constant(10),
            qubit_stmt := py.GetItem(obj=qreg.result, index=idx1.result),
            ilist.New(values=(qubit_stmt.result,)),
        ]
    )

    idx.result.hints["const"] = const.Value(0)
    idx1.result.hints["const"] = const.Value(10)
    rule = rewrite.Walk(ilist.rewrite.InlineGetItem())
    rule.rewrite(test_block)

    expected_block = ir.Block(
        [
            qreg := ilist.New(values=items),
            idx := py.Constant(0),
            qubit_stmt := py.GetItem(obj=qreg.result, index=idx.result),
            ilist.New(values=(items[0],)),
            idx1 := py.Constant(10),
            qubit_stmt := py.GetItem(obj=qreg.result, index=idx1.result),
            ilist.New(values=(qubit_stmt.result,)),
        ]
    )

    assert test_block.is_equal(expected_block)


def test_inline_getitem_slice():
    values = tuple(ir.TestValue() for _ in range(6))
    test_block = ir.Block(
        [
            qreg := ilist.New(values=values),
            slice_value := py.Constant(slice(2, 5, 1)),
            py.GetItem(obj=qreg.result, index=slice_value.result),
        ]
    )
    slice_value.result.hints["const"] = const.Value(slice(2, 5, 1))

    rule = rewrite.Walk(ilist.rewrite.InlineGetItem())
    rule.rewrite(test_block)

    expected_block = ir.Block(
        [
            qreg := ilist.New(values=values),
            slice_value := py.Constant(slice(2, 5, 1)),
            ilist.New(values=(values[2], values[3], values[4])),
        ]
    )
    assert test_block.is_equal(expected_block)


def test_ilist_flatten_add_rhs_empty():
    values = tuple(ir.TestValue() for _ in range(6))
    test_block = ir.Block(
        [
            lhs := ilist.New(values=values),
            rhs := py.Constant(ilist.IList([])),
            py.Add(lhs=lhs.result, rhs=rhs.result),
        ]
    )
    rhs.result.hints["const"] = const.Value(ilist.IList([]))

    rule = rewrite.Walk(ilist.rewrite.FlattenAdd())
    rule.rewrite(test_block)

    expected_block = ir.Block(
        [
            lhs := ilist.New(values=values),
            rhs := py.Constant(ilist.IList([])),
            ilist.New(values=values),
        ]
    )
    assert test_block.is_equal(expected_block)


def test_ilist_flatten_add_lhs_empty():
    values = tuple(ir.TestValue() for _ in range(6))
    test_block = ir.Block(
        [
            lhs := py.Constant(ilist.IList([])),
            rhs := ilist.New(values=values),
            py.Add(lhs=lhs.result, rhs=rhs.result),
        ]
    )
    lhs.result.hints["const"] = const.Value(ilist.IList([]))

    rule = rewrite.Walk(ilist.rewrite.FlattenAdd())
    rule.rewrite(test_block)

    expected_block = ir.Block(
        [
            lhs := py.Constant(ilist.IList([])),
            rhs := ilist.New(values=values),
            ilist.New(values=values),
        ]
    )
    assert test_block.is_equal(expected_block)


def test_ilist_flatten_add_lhs_not_empty():
    values = tuple(ir.TestValue() for _ in range(6))
    test_block = ir.Block(
        [
            lhs := py.Constant(value := ilist.IList([1])),
            rhs := ilist.New(values=values),
            py.Add(lhs=lhs.result, rhs=rhs.result),
        ]
    )
    lhs.result.hints["const"] = const.Value(value)

    rule = rewrite.Walk(ilist.rewrite.FlattenAdd())
    result = rule.rewrite(test_block)

    assert not result.has_done_something


def test_ilist_flatten_add_rhs_not_empty():
    values = tuple(ir.TestValue() for _ in range(6))
    test_block = ir.Block(
        [
            lhs := ilist.New(values=values),
            rhs := py.Constant(value := ilist.IList([1])),
            py.Add(lhs=lhs.result, rhs=rhs.result),
        ]
    )
    rhs.result.hints["const"] = const.Value(value)

    rule = rewrite.Walk(ilist.rewrite.FlattenAdd())
    result = rule.rewrite(test_block)

    assert not result.has_done_something


def test_ilist_flatten_add_both_new():
    lhs_values = tuple(ir.TestValue() for _ in range(6))
    rhs_values = tuple(ir.TestValue() for _ in range(3))
    test_block = ir.Block(
        [
            lhs := ilist.New(values=lhs_values),
            rhs := ilist.New(values=rhs_values),
            py.Add(lhs=lhs.result, rhs=rhs.result),
        ]
    )
    expected_block = ir.Block(
        [
            lhs := ilist.New(values=lhs_values),
            rhs := ilist.New(values=rhs_values),
            ilist.New(values=lhs_values + rhs_values),
        ]
    )
    rule = rewrite.Walk(ilist.rewrite.FlattenAdd())
    rule.rewrite(test_block)

    assert test_block.is_equal(expected_block)


def test_region_boundary_structural():

    # Do not optimize across region boundary like if-else or basic blocks
    @structural
    def test_impl(n: int):
        a = ilist.IList([])

        if n > 0:
            a = a + [n]

        return a

    expected_impl = test_impl.similar()
    test_impl.print(hint="const")
    rule = rewrite.Walk(ilist.rewrite.FlattenAdd())
    rule.rewrite(test_impl.code)

    assert test_impl.code.is_structurally_equal(expected_impl.code)


def test_region_boundary():

    # Do not optimize across region boundary like if-else or basic blocks
    @basic_no_opt
    def test_impl(n: int):
        a = ilist.IList([])

        if n > 0:
            a = a + [n]

        return a

    expected_impl = test_impl.similar()
    test_impl.print(hint="const")
    rule = rewrite.Walk(ilist.rewrite.FlattenAdd())
    rule.rewrite(test_impl.code)

    assert test_impl.code.is_structurally_equal(expected_impl.code)


def test_ilist_constprop():
    from kirin.analysis import const

    @basic_no_opt
    def test_impl(x: float) -> float:
        return x * 0.3

    @basic_no_opt
    def _for_loop(
        values: ilist.IList[float, Any],
    ) -> ilist.IList[float, Any]:

        def gen(i: int):
            return test_impl(
                x=values[i],
            )

        n_range = len(values)
        return ilist.map(fn=gen, collection=ilist.range(n_range))

    @basic_no_opt
    def main():
        values = [1.0, 2.0, 3.0]
        return _for_loop(values)  # type: ignore

    prop = const.Propagate(basic_no_opt)
    frame, result = prop.run_analysis(main)
    target_ssa = main.callable_region.blocks[0].stmts.at(-2).results[0]
    target = frame.entries[target_ssa]
    assert isinstance(target, const.Value)
    for x, y in zip(target.data, ilist.IList([0.3, 0.6, 0.9])):
        assert abs(x - y) < 1e-9

    @basic_no_opt
    def add(x, y):
        return x + y

    @basic_no_opt
    def foldl(xs):
        return ilist.foldl(add, xs, init=0)

    @basic_no_opt
    def foldr(xs):
        return ilist.foldr(add, xs, init=0)

    @basic_no_opt
    def main2():
        values = [1, 2, 3]
        return foldl(values), foldr(values)

    prop = const.Propagate(basic_no_opt)
    frame, result = prop.run_analysis(main2)
    target_ssa = main2.callable_region.blocks[0].stmts.at(-2).results[0]
    target = frame.entries[target_ssa]
    assert isinstance(target, const.Value)
    assert target.data == (6, 6)


def test_ilist_constprop_non_pure():

    new_dialect = ir.Dialect("test")

    @statement(dialect=new_dialect)
    class DefaultInit(ir.Statement):
        name = "test"
        traits = frozenset({FromPythonCall()})
        result: ir.ResultValue = info.result(types.Float)

    dialect_group = basic_no_opt.add(new_dialect)

    @dialect_group
    def test():

        def inner(_: int):
            return DefaultInit()

        return ilist.map(inner, ilist.range(10))

    _, res = const.Propagate(dialect_group).run_analysis(test)

    assert isinstance(res, const.Unknown)


rule = rewrite.Fixpoint(rewrite.Walk(ilist.rewrite.Unroll()))
xs = ilist.IList([1, 2, 3])


@basic
def map(xs: ilist.IList[int, Literal[3]]):
    return ilist.map(add1, xs)


@basic_no_opt
def foreach(xs: ilist.IList[int, Literal[3]]):
    ilist.for_each(add1, xs)


map_before = map(xs)
foreach_before = foreach(xs)
rule.rewrite(map.code)
rule.rewrite(foreach.code)
map_after = map(xs)
foreach_after = foreach(xs)
