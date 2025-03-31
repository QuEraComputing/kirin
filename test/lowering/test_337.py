from kirin.prelude import basic_no_opt
from kirin.dialects import cf, func
from kirin.lowering import Lowering


def test_issue_337():
    def test_if_inside_for() -> int:
        count = 0
        for i in range(5):
            count = count + 1
            if True:
                count = count + 100
            else:
                count = count + 300
        return count

    lowering = Lowering(basic_no_opt)
    code = lowering.run(test_if_inside_for, compactify=True)
    assert isinstance(code, func.Function)
    loop_last_block = code.body.blocks[4]
    count_5 = loop_last_block.args[0]
    stmt = loop_last_block.stmts.at(-1)
    assert isinstance(stmt, cf.ConditionalBranch)
    assert stmt.then_arguments[0] is count_5
    assert stmt.else_arguments[1] is count_5
