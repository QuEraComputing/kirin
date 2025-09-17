from pytest import mark

from kirin import types
from kirin.prelude import structural_no_opt
from kirin.analysis import TypeInference

type_infer = TypeInference(structural_no_opt)


@mark.xfail(reason="for with early return not supported in scf lowering")
def test_inside_return_loop():
    @structural_no_opt
    def simple_loop(x: float):
        for i in range(0, 3):
            return i
        return x

    frame, ret = type_infer.run(simple_loop)
    assert ret.is_subseteq(types.Int | types.Float)


@mark.xfail(reason="if with early return not supported in scf lowering")
def test_simple_ifelse():
    @structural_no_opt
    def simple_ifelse(x: int):
        cond = x > 0
        if cond:
            return cond
        else:
            return 0

    frame, ret = type_infer.run(simple_ifelse)
    assert ret.is_subseteq(types.Bool | types.Int | types.NoneType)
