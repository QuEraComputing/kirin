from kirin.passes import aggressive
from kirin.prelude import basic


def test_aggressive_inline():

    @basic(aggressive=False)
    def foo(arg0, arg1):
        return arg0 - arg1

    @basic(aggressive=False)
    def main_aggressive(arg0):
        return foo(arg1=2, arg0=arg0)

    main_aggressive.print()

    main_aggressive = main_aggressive.similar()
    aggressive.Fold(main_aggressive.dialects).fixpoint(main_aggressive)

    main_aggressive.print()

    print(main_aggressive(1))
    assert main_aggressive(1) == -1


def test_aggressive_inline_noargs():

    @basic(aggressive=False)
    def foo(arg0, arg1):
        return arg0 - arg1

    @basic(aggressive=True)
    def main_aggressive():
        return foo(arg1=2, arg0=1)

    main_aggressive.print()
    print(main_aggressive())
    assert main_aggressive() == -1


#
# def test_aggressive_inline_closure():
#     @basic(aggressive=True)
#     def main_aggressive():
#         def foo(arg0, arg1):
#             return arg0 - arg1
#
#         return foo(arg1=2, arg0=1)
#
#
#
#     main_aggressive.print()
#     assert main_aggressive() == -1
#     print(main_aggressive())
