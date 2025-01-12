from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint
from kirin.analysis import const
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.fold import ConstantFold


@basic_no_opt
def foldable(x: int) -> int:
    y = 1
    b = y + 2
    c = y + b
    d = c + 4
    return d + x


def test_dce():
    before = foldable(1)
    const_prop = const.Propagate(foldable.dialects)
    const_prop.eval(foldable, tuple(const.JointResult.top() for _ in foldable.args))
    fold = ConstantFold(const_prop.results)
    Fixpoint(Walk(fold)).rewrite(foldable.code)

    foldable.code.print()
    dce = DeadCodeElimination(const_prop.results)
    Fixpoint(Walk(dce)).rewrite(foldable.code)
    foldable.code.print()

    after = foldable(1)

    assert before == after
