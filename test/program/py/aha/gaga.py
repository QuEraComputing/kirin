from kirin.prelude import basic_no_opt


@basic_no_opt
def foo(x: int) -> int:
    assert x == 1
    return x + 1


@basic_no_opt
def goo(x: int) -> int:
    return x + 1
