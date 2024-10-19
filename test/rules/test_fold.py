from kirin.analysis.dataflow.constprop import ConstProp, ConstPropBottom
from kirin.prelude import basic_no_opt
from kirin.rewrite import Fixpoint, Walk
from kirin.rules.fold import ConstantFold


@basic_no_opt
def foldable(x: int) -> int:
    y = 1
    b = y + 2
    c = y + b
    d = c + 4
    return d + x


def test_const_fold():
    before = foldable(1)
    const_prop = ConstProp(foldable.dialects)
    const_prop.eval(foldable, tuple(ConstPropBottom() for _ in foldable.args))
    fold = ConstantFold(const_prop.results)
    Fixpoint(Walk(fold)).rewrite(foldable.code)
    after = foldable(1)

    assert before == after
