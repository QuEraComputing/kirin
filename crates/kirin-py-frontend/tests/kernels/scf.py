# Kernels exercising the `scf` dialect lowering: `if/else`, `for`-range loops
# with single and multiple carried accumulators, and nested loops.


def abs_val(x: int) -> int:
    if x < 0:
        r = 0 - x
    else:
        r = x
    return r


def sum_to(n: int) -> int:
    s = 0
    for i in range(0, n):
        s = s + i
    return s


def sum_squares(n: int) -> int:
    total = 0
    for i in range(0, n):
        total = total + i * i
    return total


def geometric(n: int, x: int) -> int:
    # two accumulators carried through the loop
    acc = 0
    power = 1
    for i in range(0, n):
        acc = acc + power
        power = power * x
    return acc


def matrix_count(rows: int, cols: int) -> int:
    total = 0
    for i in range(0, rows):
        for j in range(0, cols):
            total = total + 1
    return total
