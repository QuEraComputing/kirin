from kirin import types
from kirin.prelude import structural_no_opt
from kirin.analysis import TypeInference


def test_self_ref_closure():

    @structural_no_opt
    def should_work(n_qubits: int):
        def self_ref_source(i_layer):
            stride = n_qubits // (2**i_layer)
            if stride == 0:
                return

            self_ref_source(i_layer + 1)

        return self_ref_source

    infer = TypeInference(structural_no_opt)
    _, ret = infer.run_analysis(should_work, (types.Int,))
    assert ret.is_equal(types.MethodType[types.Tuple[types.Any], types.NoneType])
