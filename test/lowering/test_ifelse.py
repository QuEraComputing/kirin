from kirin import lowering
from kirin.prelude import python_no_opt
from kirin.dialects import cf, func

lower = lowering.Python(python_no_opt)


def test_pass():
    def nobody():
        pass

    code = lower.python_function(nobody)
    assert isinstance(code, func.Function)
    assert isinstance(code.body.blocks[-1].last_stmt, func.Return)

    def branch_pass():
        if True:
            pass
        else:
            pass

    code = lower.python_function(branch_pass)
    code.print()
    assert isinstance(code, func.Function)
    assert isinstance(code.body.blocks[0].last_stmt, func.Return)


def test_basic_ifelse():
    def single(n):
        if n == 0:
            return 1
        else:
            return n

    code = lower.python_function(single)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 3
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, func.Return)
    assert isinstance(code.body.blocks[2].last_stmt, func.Return)

    def single_2(n):
        if n == 0:
            n + 1
        else:
            return n

    code = lower.python_function(single_2)
    code.print()
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 3
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, func.Return)
    assert isinstance(code.body.blocks[2].last_stmt, func.Return)

    def single_3(n):
        if n == 0:
            n = n + 1
        else:
            n = n + 2

    code = lower.python_function(single_3)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 4
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, cf.Branch)
    assert code.body.blocks[1].last_stmt.successor is code.body.blocks[3]
    assert isinstance(code.body.blocks[2].last_stmt, cf.Branch)
    assert code.body.blocks[2].last_stmt.successor is code.body.blocks[3]
    assert isinstance(code.body.blocks[3].last_stmt, func.Return)

    def single_4(n):
        if n == 0:
            n = n + 1

    code = lower.python_function(single_4)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 3
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, cf.Branch)
    assert code.body.blocks[1].last_stmt.successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[2].last_stmt, func.Return)


def test_recursive_ifelse():
    def multi(n):
        if n == 0:
            return 1
        else:
            if n < 5:
                assert n < 6, "n must be less than 10"
            else:
                return n

    code = lower.python_function(multi)
    code.print()
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 5
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, func.Return)
    assert isinstance(code.body.blocks[2].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[2].last_stmt.then_successor is code.body.blocks[3]
    assert code.body.blocks[2].last_stmt.else_successor is code.body.blocks[4]
    assert isinstance(code.body.blocks[3].last_stmt, func.Return)
    assert isinstance(code.body.blocks[4].last_stmt, func.Return)
