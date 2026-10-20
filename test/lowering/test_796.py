from kirin import lowering
from kirin.prelude import basic_no_opt


@basic_no_opt
def double(x: int) -> int:
    return 2 * x


def factory(double: int):
    @basic_no_opt
    def scale(x: int) -> int:
        return double * x

    return scale


def test_796_closure_does_not_overwrite_module_global():
    original = globals()["double"]
    scale = factory(3)

    # module-level kernel is untouched
    assert globals()["double"] is original
    # closure variable still shadows the module global inside the kernel
    assert scale(5) == 15
    assert double(5) == 10


def test_796_caller_globals_not_mutated():
    y = 1

    def inner(x: int) -> int:
        return x + y

    caller_globals = {"z": 2}
    lowering.Python(basic_no_opt).python_function(inner, globals=caller_globals)
    assert caller_globals == {"z": 2}
