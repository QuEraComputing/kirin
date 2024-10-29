from kirin.passes.inline import InlinePass
from kirin.prelude import basic_no_opt


@basic_no_opt
def testfunc2(x: int):
    return x - 1


def test_inline_pass():

    @basic_no_opt
    def main_inline_pass(x: int):
        y = testfunc2(x)
        return y + 1

    inline = InlinePass(main_inline_pass.dialects)
    a = main_inline_pass(1)
    main_inline_pass.code.print()
    inline(main_inline_pass)
    main_inline_pass.code.print()
    b = main_inline_pass(1)
    assert a == b
    assert len(main_inline_pass.callable_region.blocks[0].stmts) == 5


def test_inline_pass_custom_heru():

    @basic_no_opt
    def main_inline_pass2(x: int):
        y = testfunc2(x)
        return y + 1

    inline = InlinePass(main_inline_pass2.dialects, herustic=lambda x: False)
    a = main_inline_pass2(1)
    main_inline_pass2.code.print()
    inline(main_inline_pass2)
    main_inline_pass2.code.print()
    b = main_inline_pass2(1)
    assert a == b

    assert len(main_inline_pass2.callable_region.blocks[0].stmts) == 4
