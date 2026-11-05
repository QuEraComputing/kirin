from kirin.passes import HintConst
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Inline, ConstantFold
from kirin.analysis import const
from kirin.dialects import func


def test_unknown_callee_hint_refines_after_inlining():
    @basic_no_opt
    def one():
        return 1

    @basic_no_opt
    def call_it(g):
        return g()

    @basic_no_opt
    def main():
        return call_it(one)

    _, result = const.Propagate(basic_no_opt).run(call_it)
    assert result == const.Unknown()
    HintConst(basic_no_opt).unsafe_run(call_it)
    Walk(Inline(lambda _: True)).rewrite(main.code)
    call = next(stmt for stmt in main.code.walk() if isinstance(stmt, func.Call))
    assert call.result.hints["const"] == const.Unknown()

    HintConst(basic_no_opt).unsafe_run(main)
    assert call.result.hints["const"] == const.Value(1)
    assert Walk(ConstantFold()).rewrite(main.code).has_done_something
    assert main() == 1
