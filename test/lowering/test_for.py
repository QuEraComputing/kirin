from kirin import lowering
from kirin.prelude import basic, basic_no_opt
from kirin.dialects import cf, func

lower = lowering.Python(basic_no_opt)


def test_simple_loop():
    def simple_loop(x):
        for i in range(10):
            for j in range(10):
                x = x + i + j

    code = lower.python_function(simple_loop)
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


def test_assign_only_variable_is_carried():
    for prelude in (basic_no_opt, basic):

        @prelude
        def last(n: int):
            x = 0
            for i in range(n):
                x = 1
            return x

        assert last(0) == 0
        assert last(1) == 1
        assert last(3) == 1


def test_assign_only_from_loop_variable():
    for prelude in (basic_no_opt, basic):

        @prelude
        def last_value(n: int):
            x = -1
            for i in range(n):
                x = i
            return x

        assert last_value(0) == -1
        assert last_value(1) == 0
        assert last_value(4) == 3


def test_multiple_assign_only_variables():
    for prelude in (basic_no_opt, basic):

        @prelude
        def main(n: int):
            x = 0
            y = 0
            for i in range(n):
                x = 1
                y = 2
            return x * 10 + y

        assert main(0) == 0
        assert main(3) == 12


def test_mixed_read_write_and_assign_only_variables():
    for prelude in (basic_no_opt, basic):

        @prelude
        def main(n: int):
            x = 0
            y = 0
            for i in range(n):
                x = x + 1
                y = 7
            return x * 10 + y

        assert main(0) == 0
        assert main(3) == 37


def test_assign_only_in_nested_loops():
    for prelude in (basic_no_opt, basic):

        @prelude
        def main(n: int, m: int):
            x = 0
            for i in range(n):
                for j in range(m):
                    x = 1
            return x

        assert main(0, 3) == 0
        assert main(3, 0) == 0
        assert main(3, 2) == 1


def test_conditional_assignment_in_loop():
    for prelude in (basic_no_opt, basic):

        @prelude
        def main(n: int):
            x = 0
            for i in range(n):
                if i > 1:
                    x = i
            return x

        assert main(0) == 0
        assert main(2) == 0
        assert main(5) == 4


def test_assign_only_loop_inside_if():
    for prelude in (basic_no_opt, basic):

        @prelude
        def main(cond: bool, n: int):
            x = 0
            if cond:
                for i in range(n):
                    x = 1
            return x

        assert main(False, 3) == 0
        assert main(True, 0) == 0
        assert main(True, 3) == 1


def test_preexisting_loop_target_and_body_only_temporary():
    for prelude in (basic_no_opt, basic):

        @prelude
        def main(n: int):
            i = 99
            x = 0
            for i in range(n):
                tmp = 1
                x = tmp
            return i * 10 + x

        assert main(0) == 990
        assert main(3) == 21


def test_assign_only_ir_carries_value_through_loop():
    def assign_only(n):
        x = 0
        for i in range(n):
            x = 1
        return x

    code = lower.python_function(assign_only)
    code.verify()
    assert isinstance(code, func.Function)
    entry, body, exit_block = code.body.blocks

    # the pre-loop value reaches the exit on the empty-iterator path and
    # enters the body as a block argument on the non-empty path
    initial_x = entry.stmts.at(0).results[0]
    assert isinstance(stmt := entry.last_stmt, cf.ConditionalBranch)
    assert stmt.then_successor is exit_block
    assert stmt.then_arguments[0] is initial_x
    assert stmt.else_successor is body
    assert stmt.else_arguments[1] is initial_x
    assert body.args[1].name == "x"

    # the value assigned in the body is yielded on both the exit path and
    # the back-edge, and the exit block argument is what gets returned
    assigned_x = body.stmts.at(0).results[0]
    assert isinstance(stmt := body.last_stmt, cf.ConditionalBranch)
    assert stmt.then_successor is exit_block
    assert stmt.then_arguments[0] is assigned_x
    assert stmt.else_successor is body
    assert stmt.else_arguments[1] is assigned_x
    assert exit_block.args[0].name == "x"
    assert isinstance(exit_block.last_stmt, func.Return)
    assert exit_block.last_stmt.args[0] is exit_block.args[0]


def test_branch_pass():
    def branch_pass():
        if True:
            pass
        else:
            pass

    code = lower.python_function(branch_pass)
    assert isinstance(code, func.Function)
    assert isinstance(code.body.blocks[0].last_stmt, func.Return)
    # code.print()


def test_side_effect():
    def side_effect(reg, n: int):
        if n == 0:
            return

        for i in range(10):
            reg[0] = i

    code = lower.python_function(side_effect)
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
