from kirin.prelude import basic_no_opt, python_no_opt
from kirin.rewrite import Walk, Fixpoint, WrapConst
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
    frame, _ = const_prop.run_analysis(foldable)
    Fixpoint(Walk(WrapConst(frame))).rewrite(foldable.code)
    fold = ConstantFold()
    Fixpoint(Walk(fold)).rewrite(foldable.code)
    after = foldable(1)

    assert before == after


def test_const_fold_subroutine():

    @python_no_opt
    def non_pure_subroutine(x: list[int]) -> None:
        x.append(1)

    @python_no_opt
    def main():
        x = []
        non_pure_subroutine(x)
        x.append(2)

    old_main_region = main.callable_region.clone()

    fold = ConstantFold()
    Fixpoint(Walk(fold)).rewrite(main.code)

    assert old_main_region.is_equal(main.callable_region)
