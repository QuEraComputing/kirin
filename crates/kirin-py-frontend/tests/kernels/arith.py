# Kernels exercising the `arith` + `constant` dialect lowering.
# Integer-only (the float path is a documented v1 limitation).


def poly(x: int) -> int:
    # 3*x^2 + 2*x + 7
    return x * x * 3 + x * 2 + 7


def mix(a: int, b: int, c: int) -> int:
    t = a * b - c
    u = t + a
    return u * b - a


def affine(x: int) -> int:
    return (x + 1) * (x - 1)


def remainder(a: int, b: int) -> int:
    q = a / b
    return a - q * b
