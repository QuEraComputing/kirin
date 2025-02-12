from kirin import types
from kirin.prelude import basic


@basic
def foo(x: int):
    if x > 1:
        return x + 1
    else:
        return x - 1.0


@basic(typeinfer=True)
def main(x: int):
    return foo(x)


@basic(typeinfer=True)
def moo(x):
    return foo(x)


def test_inter_method_infer():
    assert main.return_type == (types.Int | types.Float)
    # assert moo.arg_types[0] == types.Int  # type gets narrowed based on callee
    assert moo.return_type == (types.Int | types.Float)
    # NOTE: inference of moo should not update foo
    assert foo.arg_types[0] == types.Int
    assert foo.inferred is False
    assert foo.return_type is types.Any
