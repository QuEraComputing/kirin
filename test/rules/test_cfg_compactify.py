from kirin import ir
from kirin.analysis.cfg import CFG
from kirin.dialects import cf, func
from kirin.dialects.func import Lambda
from kirin.dialects.py import stmts, types
from kirin.prelude import basic_no_opt
from kirin.rewrite import Fixpoint, Walk
from kirin.rules.cfg_compatify import CFGCompactify
from kirin.rules.inline import Inline


@basic_no_opt
def foo(x: int):  # type: ignore
    def goo(y: int):
        return x + y

    return goo


def test_cfg_compactify():
    cfg = CFG(foo.callable_region)
    compactify = CFGCompactify(cfg)
    Fixpoint(compactify).rewrite(foo.code)
    foo.callable_region.blocks[0].stmts.at(1).print()
    assert len(foo.callable_region.blocks[0].stmts) == 2
    stmt = foo.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, Lambda)
    assert len(stmt.body.blocks[0].stmts) == 3
    assert len(stmt.body.blocks) == 1


@basic_no_opt
def my_func(x: int, y: int):
    def foo(a: int, b: int):
        return a + b + x + y

    return foo


@basic_no_opt
def my_main_test_cfg():
    a = 3
    b = 4
    c = my_func(1, 2)
    return c(a, b) * 4


def test_compactify_replace_block_arguments():
    Walk(Inline(heuristic=lambda x: True)).rewrite(my_main_test_cfg.code)
    cfg = CFG(my_main_test_cfg.callable_region)
    compactify = CFGCompactify(cfg)
    Fixpoint(compactify).rewrite(my_main_test_cfg.code)
    my_main_test_cfg.code.print()
    stmt = my_main_test_cfg.callable_region.blocks[0].stmts.at(5)
    assert isinstance(stmt, func.Lambda)
    assert isinstance(stmt.captured[0].owner, stmts.Constant)
    assert stmt.captured[0].name == "x"
    assert isinstance(stmt.captured[1].owner, stmts.Constant)
    assert stmt.captured[1].name == "y"


def test_compactify_single_branch_block():
    region = ir.Region()
    region.blocks.append(ir.Block())
    region.blocks.append(ir.Block())
    region.blocks.append(ir.Block())
    region.blocks.append(ir.Block())
    region.blocks[0].args.append_from(types.Any, "self")
    x = region.blocks[0].args.append_from(types.Any, "x")
    const_0 = stmts.Constant(0)
    const_n = stmts.Constant(3)
    const_n.result.name = "n"
    cond = stmts.Eq(x, const_0.result)
    cond.result.name = "cond"
    region.blocks[0].stmts.append(const_0)
    region.blocks[0].stmts.append(const_n)
    region.blocks[0].stmts.append(cond)
    region.blocks[0].stmts.append(
        cf.ConditionalBranch(
            cond.result,
            then_arguments=(),
            then_successor=region.blocks[1],
            else_arguments=(),
            else_successor=region.blocks[2],
        )
    )
    region.blocks[1].stmts.append(
        cf.Branch(arguments=(const_n.result,), successor=region.blocks[3])
    )
    const_1 = stmts.Constant(1)
    sub = stmts.Sub(x, const_1.result)
    region.blocks[2].stmts.append(const_1)
    region.blocks[2].stmts.append(sub)
    region.blocks[2].stmts.append(
        cf.Branch(arguments=(sub.result,), successor=region.blocks[3])
    )
    z = region.blocks[3].args.append_from(types.Any, "z")
    mul = stmts.Mult(x, z)
    region.blocks[3].stmts.append(mul)
    region.blocks[3].stmts.append(func.Return(mul.result))
    cfg = CFG(region)
    compactify = CFGCompactify(cfg)
    compactify.rewrite(region)
    assert len(region.blocks) == 3
    stmt = region.blocks[0].last_stmt
    assert isinstance(stmt, cf.ConditionalBranch)
    assert stmt.then_successor is region.blocks[2]
