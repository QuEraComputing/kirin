from kirin import types
from kirin.prelude import basic


def test_global_hint():
    @basic
    def main(xs: types.Float) -> None:  # type: ignore
        return None

    assert main.code.signature.inputs[0] == types.Float  # type: ignore
    assert main.code.signature.output == types.NoneType  # type: ignore
