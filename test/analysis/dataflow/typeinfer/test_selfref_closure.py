from pytest import mark

from kirin import types
from kirin.prelude import structural_no_opt
from kirin.analysis import TypeInference


@mark.xfail(reason="if with early return not supported in scf lowering")
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
    frame, ret = infer.run(should_work, types.Int)
    should_work.print(analysis=frame.entries)
    assert ret.is_structurally_equal(
        types.MethodType[types.Tuple[types.Any], types.NoneType]
    )
