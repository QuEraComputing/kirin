from kirin import types
from kirin.prelude import structural_no_opt
from kirin.analysis import TypeInference


def test_inside_return_loop():
    @structural_no_opt
    def simple_loop(x: float):
        for i in range(0, 3):
            return i
        return x

    type_infer = TypeInference(structural_no_opt)
    frame, ret = type_infer.run_analysis(simple_loop)
    assert ret.is_subseteq(types.Int | types.Float)
