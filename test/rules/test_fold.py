from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint
from kirin.analysis import const
from kirin.rewrite.fold import ConstantFold


@basic_no_opt
def foldable(x: int) -> int:
    y = 1
    b = y + 2
    c = y + b
    d = c + 4
    return d + x


def test_const_fold():
    before = foldable(1)
    const_prop = const.Propagate(foldable.dialects)
    results, _ = const_prop.run_analysis(foldable)
    fold = ConstantFold(results)
    Fixpoint(Walk(fold)).rewrite(foldable.code)
    after = foldable(1)

    assert before == after
