from kirin.passes import aggressive
from kirin.prelude import basic


def test_aggressive_inline():

    @basic(aggressive=False)
    def foo0(arg0, arg1):
        return arg0 - arg1

    @basic(aggressive=False)
    def main_aggressive(arg0):
        return foo0(arg1=2, arg0=arg0)

    main_aggressive = main_aggressive.similar()
    aggressive.Fold(main_aggressive.dialects).fixpoint(main_aggressive)

    assert main_aggressive(1) == -1


def test_aggressive_inline_noargs():

    @basic(aggressive=False)
    def foo1(arg0, arg1):
        return arg0 - arg1

    @basic(aggressive=True)
    def main_aggressive():
        return foo1(arg1=2, arg0=1)

    assert main_aggressive() == -1


def test_aggressive_inline_pos_args():

    @basic(aggressive=False)
    def foo2(arg0, arg1):
        return arg0 - arg1

    @basic(aggressive=True)
    def main_aggressive(arg0):
        return foo2(arg0, 2)

    assert main_aggressive(1) == -1


def test_aggressive_inline_closure():

    # @basic(aggressive=False, fold=False, typeinfer=True)
    @basic
    def main_aggressive(param: int):
        def foo3(arg0: int, arg1: int):
            return arg0 - arg1 + param

        return foo3(arg1=2, arg0=1)

    main_aggressive = main_aggressive.similar()
    aggressive.Fold(main_aggressive.dialects).fixpoint(main_aggressive)

    assert main_aggressive(1) == 0


def test_aggressive_inline_closure_pos_args():

    # @basic(aggressive=False, fold=False, typeinfer=True)
    @basic
    def main_aggressive(param: int):
        def foo3(arg0: int, arg1: int):
            return arg0 - arg1 + param

        return foo3(1, arg1=2)

    main_aggressive = main_aggressive.similar()
    aggressive.Fold(main_aggressive.dialects).fixpoint(main_aggressive)

    assert main_aggressive(1) == 0


def test_aggressive_inline_closure_alias():
    @basic(aggressive=True)
    def main_aggressive2(param: int):
        def foo4(arg0: int, arg1: int):
            return arg0 - arg1 + param

        alias_foo4 = foo4

        return alias_foo4(arg1=2, arg0=1)

    assert main_aggressive2(1) == 0
