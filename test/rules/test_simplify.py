from kirin.interp import Interpreter
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk
from kirin.rules.simplify import AliasSimplifyRewrite, GetitemSimplifyRewrite


@basic_no_opt
def main_simplify_alias(x: int):
    y = x + 1
    z = y
    z2 = z
    return z2


@basic_no_opt
def main_simplify_getitem(x: int):
    ylist = (x, x, 1, 2)
    return ylist[0]


def test_simplify():

    before = main_simplify_alias(1)
    asr = AliasSimplifyRewrite(interp=Interpreter(main_simplify_alias.dialects))
    main_simplify_alias.code.print()
    Walk(asr).rewrite(main_simplify_alias.code)
    main_simplify_alias.code.print()
    after = main_simplify_alias(1)

    assert before == after


def test_simplify_getitem():

    before = main_simplify_getitem(1)
    asr = GetitemSimplifyRewrite(interp=Interpreter(main_simplify_getitem.dialects))
    main_simplify_getitem.code.print()
    Walk(asr).rewrite(main_simplify_getitem.code)
    main_simplify_getitem.code.print()
    after = main_simplify_getitem(1)

    assert before == after
