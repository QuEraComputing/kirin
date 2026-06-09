# Kernels exercising the `cmp` dialect lowering (all six comparison ops),
# plus nested `if/else` that consume comparison results.


def is_eq(a: int, b: int) -> int:
    return a == b


def is_ne(a: int, b: int) -> int:
    return a != b


def is_lt(a: int, b: int) -> int:
    return a < b


def is_le(a: int, b: int) -> int:
    return a <= b


def is_gt(a: int, b: int) -> int:
    return a > b


def is_ge(a: int, b: int) -> int:
    return a >= b


def clamp(x: int, lo: int, hi: int) -> int:
    if x < lo:
        r = lo
    else:
        if x > hi:
            r = hi
        else:
            r = x
    return r
