from kirin import ir
from kirin.analysis.dataflow.constprop import Const, ConstProp, NotConst, PartialTuple
from kirin.prelude import basic_no_opt


@basic_no_opt
def foo(x):
    return x + 1


@basic_no_opt
def goo(x):
    return foo(2), foo(x)


@basic_no_opt
def main():
    return goo(3)


@basic_no_opt
def bar(x):
    return goo(x)[0]


def test_constprop():
    infer = ConstProp(basic_no_opt)
    assert infer.eval(main, tuple(NotConst() for _ in main.args)).expect() == Const(
        (3, 4)
    )
    assert len(infer.results) == 4

    assert infer.eval(
        goo, tuple(NotConst() for _ in goo.args)
    ).expect() == PartialTuple((Const(3), NotConst()))
    assert len(infer.results) == 8
    block: ir.Block = goo.code.body.blocks[0]  # type: ignore
    assert infer.results[block.stmts.at(2).results[0]] == Const(3)
    assert infer.results[block.stmts.at(4).results[0]] == NotConst()
    assert infer.results[block.stmts.at(5).results[0]] == PartialTuple(
        (Const(3), NotConst())
    )

    assert infer.eval(bar, tuple(NotConst() for _ in bar.args)).expect() == Const(3)
