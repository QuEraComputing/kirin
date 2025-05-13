from kirin import ir, types
from kirin.prelude import structural
from kirin.rewrite import Walk
from kirin.dialects import cf, py, func
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
        slots=("b",),
        signature=func.Signature(
            output=types.Bool,
            inputs=(types.Bool,),
        ),
        body=excpected_callable_region,
    )

    expected_test = ir.Method(
        dialects=structural,
        code=expected_code,
    )

    if structural.run_pass is not None:
        structural.run_pass(expected_test, typeinfer=True)
        structural.run_pass(test, typeinfer=True)

    assert expected_test.callable_region.is_structurally_equal(test.callable_region)


def test_scf2cf_for_1():

    @structural(typeinfer=True, fold=False)
    def test():
        j = 0
        for i in range(10):
            j = j + 1

        return j

    test.print()

    rule = Walk(scf2cf.ScfToCfRule())
    rule.rewrite(test.code)

    test.print()

    assert False
