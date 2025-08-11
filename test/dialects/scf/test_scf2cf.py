from kirin import ir, types
from kirin.prelude import basic, structural
from kirin.rewrite import Walk
from kirin.dialects import cf, py, func, ilist
from kirin.dialects.scf import scf2cf


def test_scf2cf_if_1():

    @structural(typeinfer=True)
    def test(b: bool):
        if b:
            b = False
        else:
            b = not b

        return b

    rule = Walk(scf2cf.ScfToCfRule())
    rule.rewrite(test.code)
    test = test.similar(basic)

    excpected_callable_region = ir.Region(
        [
            branch_block := ir.Block(),
            then_block := ir.Block(),
            else_block := ir.Block(),
            join_block := ir.Block(),
        ]
    )

    branch_block.args.append_from(types.MethodType, "self")
    b = branch_block.args.append_from(types.Bool, "b")
    branch_block.stmts.append(
        cf.ConditionalBranch(
            cond=b,
            then_arguments=(b,),
            then_successor=then_block,
            else_arguments=(b,),
            else_successor=else_block,
        )
    )

    then_block.args.append_from(types.Bool, "b")
    then_block.stmts.append(stmt := py.Constant(value=False))
    then_block.stmts.append(
        cf.Branch(
            arguments=(stmt.result,),
            successor=join_block,
        )
    )

    b = else_block.args.append_from(types.Bool)
    else_block.stmts.append(stmt := py.unary.Not(b))
    else_block.stmts.append(
        cf.Branch(
            arguments=(stmt.result,),
            successor=join_block,
        )
    )
    ret = join_block.args.append_from(types.Bool)
    join_block.stmts.append(func.Return(ret))

    expected_code = func.Function(
        sym_name="test",
        signature=func.Signature(
            output=types.Bool,
            inputs=(types.Bool,),
        ),
        body=excpected_callable_region,
    )

    expected_test = ir.Method(
        dialects=basic,
        code=expected_code,
        mod=None,
        py_func=None,
        sym_name="expected_test",
        arg_names=[],
    )

    if basic.run_pass is not None:
        basic.run_pass(expected_test, typeinfer=True)
        basic.run_pass(test, typeinfer=True)

    assert expected_test.callable_region.is_structurally_equal(test.callable_region)


def test_scf2cf_for_1():

    @structural(typeinfer=True, fold=False)
    def test():
        j = 0
        for i in range(10):
            j = j + 1

        return j

    rule = Walk(scf2cf.ScfToCfRule())
    rule.rewrite(test.code)
    test = test.similar(basic)

    expected_callable_region = ir.Region(
        [
            curr_block := ir.Block(),
            entry_block := ir.Block(),
            body_block := ir.Block(),
            exit_block := ir.Block(),
        ]
    )

    curr_block.args.append_from(types.MethodType, "self")
    curr_block.stmts.append(j_start := py.Constant(value=0))

    j_start.result.name = "j"
    curr_block.stmts.append(iter_start := py.Constant(value=0))
    curr_block.stmts.append(iter_end := py.Constant(value=10))
    curr_block.stmts.append(iter_step := py.Constant(value=1))
    curr_block.stmts.append(
        range_stmt := ilist.stmts.Range(
            start=iter_start.result,
            stop=iter_end.result,
            step=iter_step.result,
        )
    )
    range_stmt.result.type = ilist.IListType[types.Int, types.Literal(10)]
    curr_block.stmts.append(
        cf.Branch(
            arguments=(),
            successor=entry_block,
        )
    )
    entry_block.stmts.append(iterable_stmt := py.iterable.Iter(range_stmt.result))
    entry_block.stmts.append(
        first_iter := py.iterable.Next(iterable_stmt.expect_one_result())
    )
    entry_block.stmts.append(none_stmt := func.ConstantNone())
    entry_block.stmts.append(
        loop_cmp := py.cmp.Is(first_iter.expect_one_result(), none_stmt.result)
    )
    entry_block.stmts.append(
        cf.ConditionalBranch(
            cond=loop_cmp.result,
            then_arguments=(j_start.result, j_start.result),
            then_successor=exit_block,
            else_arguments=(
                first_iter.expect_one_result(),
                j_start.result,
                j_start.result,
            ),
            else_successor=body_block,
        )
    )

    body_block.args.append_from(types.Int, "i")
    body_block.args.append_from(types.Int, "j")
    body_block.args.append_from(types.Int, "j")

    body_block.stmts.append(one_stmt := py.Constant(value=1))
    body_block.stmts.append(
        j_add := py.binop.Add(
            lhs=body_block.args[1],
            rhs=one_stmt.result,
        )
    )
    j_add.result.name = "j"
    j_add.result.type = types.Int
    body_block.stmts.append(
        next_iter := py.iterable.Next(iterable_stmt.expect_one_result())
    )
    body_block.stmts.append(none_stmt := func.ConstantNone())
    body_block.stmts.append(
        loop_cmp := py.cmp.Is(next_iter.expect_one_result(), none_stmt.result)
    )
    body_block.stmts.append(
        cf.ConditionalBranch(
            cond=loop_cmp.result,
            then_arguments=(j_add.result, j_add.result),
            then_successor=exit_block,
            else_arguments=(next_iter.expect_one_result(), j_add.result, j_add.result),
            else_successor=body_block,
        )
    )

    exit_block.args.append_from(types.Int, "j")
    exit_block.args.append_from(types.Int, "j")
    exit_block.stmts.append(func.Return(exit_block.args[0]))

    expected_code = func.Function(
        sym_name="test",
        signature=func.Signature(
            output=types.Literal(10),
            inputs=(),
        ),
        body=expected_callable_region,
    )

    expected_test = ir.Method(
        dialects=basic,
        code=expected_code,
        mod=None,
        py_func=None,
        sym_name="expected_test",
        arg_names=[],
    )

    test.print()
    expected_test.print()

    if basic.run_pass is not None:
        basic.run_pass(test, typeinfer=True, fold=False)
        basic.run_pass(expected_test, typeinfer=True, fold=False)

    assert expected_test.callable_region.is_structurally_equal(test.callable_region)
