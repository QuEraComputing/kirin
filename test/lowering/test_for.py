from kirin.prelude import basic_no_opt
from kirin.dialects import cf, func
from kirin.lowering import Lowering

lowering = Lowering(basic_no_opt)


def test_simple_loop():
    def simple_loop(x):
        for i in range(10):
            for j in range(10):
                x = x + i + j

    code = lowering.run(simple_loop)
    assert isinstance(code, func.Function)
    assert isinstance(stmt := code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert stmt.then_arguments[0] is code.body.blocks[0].args[1]
    assert stmt.then_successor is code.body.blocks[4]
    assert stmt.else_arguments[0] is code.body.blocks[0].stmts.at(6).results[0]
    assert stmt.else_arguments[1] is code.body.blocks[0].args[1]
    assert stmt.else_successor is code.body.blocks[1]

    assert isinstance(stmt := code.body.blocks[1].last_stmt, cf.ConditionalBranch)
    assert stmt.then_arguments[0] is code.body.blocks[1].args[1]
    assert stmt.then_arguments[1] is code.body.blocks[1].args[0]
    assert stmt.else_arguments[0] is code.body.blocks[1].stmts.at(-3).results[0]
    assert stmt.else_arguments[1] is code.body.blocks[1].args[1]
    assert stmt.else_arguments[2] is code.body.blocks[1].args[0]
    assert stmt.else_successor is code.body.blocks[2]

    assert isinstance(stmt := code.body.blocks[2].last_stmt, cf.ConditionalBranch)
    var_x = code.body.blocks[2].stmts.at(1).results[0]
    var_i = code.body.blocks[2].args[2]
    assert stmt.then_arguments[0] is var_x
    assert stmt.then_arguments[1] is var_i
    assert stmt.then_successor is code.body.blocks[3]
    assert stmt.else_arguments[0] is code.body.blocks[2].stmts.at(-3).results[0]
    assert stmt.else_arguments[1] is var_x
    assert stmt.else_arguments[2] is var_i
    # code.print()


def test_branch_pass():
    def branch_pass():
        if True:
            pass
        else:
            pass

    code = lowering.run(branch_pass)
    assert isinstance(code, func.Function)
    assert isinstance(code.body.blocks[0].last_stmt, func.Return)
    # code.print()


def test_side_effect():
    def side_effect(reg, n: int):
        if n == 0:
            return

        for i in range(10):
            reg[0] = i

    code = lowering.run(side_effect)
    assert isinstance(code, func.Function)
    assert isinstance(stmt := code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert stmt.then_arguments[0] is code.body.blocks[0].stmts.at(-2).results[0]
    assert stmt.then_successor is code.body.blocks[1]
    assert stmt.else_arguments == ()
    assert stmt.else_successor is code.body.blocks[2]

    assert isinstance(code.body.blocks[1].last_stmt, func.Return)

    assert isinstance(stmt := code.body.blocks[2].last_stmt, cf.ConditionalBranch)
    reg = code.body.blocks[0].args[1]
    assert stmt.then_arguments[0] is reg
    assert stmt.then_successor is code.body.blocks[4]
    assert stmt.else_arguments[0] is code.body.blocks[2].stmts.at(-3).results[0]
    assert stmt.else_arguments[1] is reg
    assert stmt.else_successor is code.body.blocks[3]

    assert isinstance(stmt := code.body.blocks[3].last_stmt, cf.ConditionalBranch)
    reg = code.body.blocks[3].args[1]
    assert stmt.then_arguments[0] is reg
    assert stmt.then_successor is code.body.blocks[4]
    assert stmt.else_arguments[0] is code.body.blocks[3].stmts.at(-3).results[0]
    assert stmt.else_arguments[1] is reg

    assert isinstance(code.body.blocks[4].last_stmt, func.Return)
    # code.print()
