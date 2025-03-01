from kirin import types
from kirin.prelude import basic

x = [1, 2, 3]


@basic(typeinfer=True)
def main():
    return x[1]


main.print(hint="const")


def test_const_infer():
    assert main.return_type is not None
    assert main.return_type.is_subseteq(types.Int)
