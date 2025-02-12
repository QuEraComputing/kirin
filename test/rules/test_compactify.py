from kirin import ir, types, lowering
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Chain, Fixpoint, compactify
from kirin.analysis import CFG
from kirin.dialects import cf, py, func


def test_duplicated_branch():
    code = func.Function(
        sym_name="duplicated_branch",
        signature=func.Signature((), types.NoneType),
        body=ir.Region(),
    )
    code.body.blocks.append(ir.Block())
    code.body.blocks.append(ir.Block())
    code.body.blocks.append(ir.Block())
    code.body.blocks.append(ir.Block())
    code.body.blocks.append(ir.Block())

    true = py.Constant(True)
    code.body.blocks[0].stmts.append(true)
    code.body.blocks[0].stmts.append(
        cf.ConditionalBranch(
            true.result,
            (),
            (),
            then_successor=code.body.blocks[1],
            else_successor=code.body.blocks[1],
        )
    )
    code.body.blocks[1].stmts.append(cf.Branch((), successor=code.body.blocks[2]))
    code.body.blocks[2].stmts.append(cf.Branch((), successor=code.body.blocks[3]))
    code.body.blocks[3].stmts.append(cf.Branch((), successor=code.body.blocks[4]))
    none = func.ConstantNone()
    code.body.blocks[4].stmts.append(none)
    code.body.blocks[4].stmts.append(func.Return(none.result))
    Fixpoint(
        Walk(Chain(compactify.DuplicatedBranch(), compactify.CFGEdge(CFG(code.body))))
    ).rewrite(code)

    target = func.Function(
        sym_name="duplicated_branch",
        signature=func.Signature((), types.NoneType),
        body=ir.Region(),
    )
    target.body.blocks.append(ir.Block())
    bb0 = target.body.blocks[0]
    true = py.Constant(True)
    none = func.ConstantNone()
    bb0.stmts.append(true)
    bb0.stmts.append(none)
    bb0.stmts.append(func.Return(none.result))

    assert code.is_equal(target)


def test_cfg_skip_block():
    code = func.Function(
        sym_name="cfg_double_branch",
        signature=func.Signature((), types.NoneType),
        body=ir.Region(),
    )
    code.body.blocks.append(ir.Block())
    code.body.blocks.append(ir.Block())
    code.body.blocks.append(ir.Block())
    code.body.blocks.append(ir.Block())
    code.body.blocks[0].args.append_from(types.Any, "self")
    cond = code.body.blocks[0].args.append_from(types.Any, "cond")
    a = py.Constant(1)
    b = py.Constant(2)
    c = py.Constant(3)

    code.body.blocks[0].stmts.append(a)
    code.body.blocks[0].stmts.append(b)
    code.body.blocks[0].stmts.append(c)
    code.body.blocks[0].stmts.append(
        cf.ConditionalBranch(
            cond,
            (a.result,),
            (b.result, c.result),
            then_successor=code.body.blocks[1],
            else_successor=code.body.blocks[2],
        )
    )

    bb1_a = code.body.blocks[1].args.append_from(types.Any, "a")
    code.body.blocks[1].stmts.append(cf.Branch((bb1_a,), successor=code.body.blocks[3]))

    bb2_b = code.body.blocks[2].args.append_from(types.Any, "b")
    code.body.blocks[2].args.append_from(types.Any, "c")
    code.body.blocks[2].stmts.append(cf.Branch((bb2_b,), successor=code.body.blocks[3]))

    code.body.blocks[3].args.append_from(types.Any, "x")
    none = func.ConstantNone()
    code.body.blocks[3].stmts.append(none)
    code.body.blocks[3].stmts.append(func.Return(none.result))

    Fixpoint(Walk(compactify.SkipBlock(CFG(code.body)))).rewrite(code)
    Fixpoint(Walk(compactify.CFGEdge(CFG(code.body)))).rewrite(code)
    Fixpoint(Walk(compactify.DeadBlock(CFG(code.body)))).rewrite(code)

    target = func.Function(
        sym_name="cfg_double_branch",
        signature=func.Signature((), types.NoneType),
        body=ir.Region(),
    )
    target.body.blocks.append(ir.Block())
    target.body.blocks.append(ir.Block())

    bb0 = target.body.blocks[0]
    bb1 = target.body.blocks[1]
    bb0.args.append_from(types.Any, "self")
    cond = bb0.args.append_from(types.Any, "cond")
    a = py.Constant(1)
    b = py.Constant(2)
    c = py.Constant(3)
    bb0.stmts.extend([a, b, c])
    bb0.stmts.append(
        cf.ConditionalBranch(
            cond, (a.result,), (b.result,), then_successor=bb1, else_successor=bb1
        )
    )

    bb1.args.append_from(types.Any, "x")
    none = func.ConstantNone()
    bb1.stmts.extend([none, func.Return(none.result)])
    code.print()
    target.print()
    assert code.is_equal(target)


def test_cfg_pass_around():
    def main():
        if True:
            pass
        else:
            pass

    lower = lowering.Lowering(basic_no_opt)
    code = lower.run(main, compactify=False)
    assert isinstance(code, func.Function)

    cfg = CFG(code.body)
    compactify.DeadBlock(cfg).rewrite(code.body)
    compactify.CFGEdge(cfg).rewrite(code.body)
    assert cfg.successors == CFG(code.body).successors
    compactify.CFGEdge(cfg).rewrite(code.body)
    assert cfg.successors == CFG(code.body).successors
    compactify.SkipBlock(cfg).rewrite(code.body)
    compactify.DeadBlock(cfg).rewrite(code.body)
    assert cfg.successors == CFG(code.body).successors
    Walk(compactify.DuplicatedBranch()).rewrite(code.body)
    assert cfg.successors == CFG(code.body).successors
    compactify.CFGEdge(cfg).rewrite(code.body)
    assert cfg.successors == CFG(code.body).successors
