from kirin import ir, types
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint
from kirin.dialects import cf, func
from kirin.dialects.py import cmp, binop
from kirin.analysis.cfg import CFG
from kirin.dialects.func import Lambda
from kirin.rewrite.inline import Inline
from kirin.rewrite.compactify import CFGCompactify, CompactifyRegion
from kirin.dialects.py.constant import Constant


@basic_no_opt
def foo(x: int):  # type: ignore
    def goo(y: int):
        return x + y

    return goo


def test_cfg_compactify():
    compactify = Walk(CFGCompactify())
    Fixpoint(compactify).rewrite(foo.code)
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
    compactify = Walk(CFGCompactify())
    Fixpoint(compactify).rewrite(my_main_test_cfg.code)
    stmt = my_main_test_cfg.callable_region.blocks[0].stmts.at(5)
    assert isinstance(stmt, func.Lambda)
    assert isinstance(stmt.captured[0].owner, Constant)
    assert stmt.captured[0].name == "x"
    assert isinstance(stmt.captured[1].owner, Constant)
    assert stmt.captured[1].name == "y"


def test_compactify_single_branch_block():
    region = ir.Region()
    region.blocks.append(ir.Block())
    region.blocks.append(ir.Block())
    region.blocks.append(ir.Block())
    region.blocks.append(ir.Block())
    region.blocks[0].args.append_from(types.Any, "self")
    x = region.blocks[0].args.append_from(types.Any, "x")
    const_0 = Constant(0)
    const_n = Constant(3)
    const_n.result.name = "n"
    cond = cmp.Eq(x, const_0.result)
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
    const_1 = Constant(1)
    sub = binop.Sub(x, const_1.result)
    region.blocks[2].stmts.append(const_1)
    region.blocks[2].stmts.append(sub)
    region.blocks[2].stmts.append(
        cf.Branch(arguments=(sub.result,), successor=region.blocks[3])
    )
    z = region.blocks[3].args.append_from(types.Any, "z")
    mul = binop.Mult(x, z)
    region.blocks[3].stmts.append(mul)
    region.blocks[3].stmts.append(func.Return(mul.result))
    cfg = CFG(region)
    compactify = Walk(CompactifyRegion(cfg))
    compactify.rewrite(region)
    region.print()
    assert len(region.blocks) == 3
    stmt = region.blocks[0].last_stmt
    assert isinstance(stmt, cf.ConditionalBranch)
    assert stmt.then_successor is region.blocks[2]


def test_compactify_entry_block_single_branch():
    region = ir.Region()
    region.blocks.append(ir.Block())
    region.blocks.append(ir.Block())
    region.blocks[0].args.append_from(types.Any, "self")
    x = region.blocks[0].args.append_from(types.Any, "x")
    region.blocks[0].stmts.append(cf.Branch(arguments=(), successor=region.blocks[1]))
    x_1 = Constant(0)
    x_2 = binop.Add(x, x_1.result)
    region.blocks[1].stmts.append(x_1)
    region.blocks[1].stmts.append(x_2)
    region.blocks[1].stmts.append(func.Return(x_2.result))
    cfg = CFG(region)
    compactify = Walk(CompactifyRegion(cfg))
    compactify.rewrite(region)

    target = ir.Region(ir.Block())
    target.blocks[0].args.append_from(types.Any, "self")
    x = target.blocks[0].args.append_from(types.Any, "x")
    x0 = Constant(0)
    target.blocks[0].stmts.append(x0)
    x1 = binop.Add(x, x0.result)
    target.blocks[0].stmts.append(x1)
    target.blocks[0].stmts.append(func.Return(x1.result))
    assert region.is_structurally_equal(target)
