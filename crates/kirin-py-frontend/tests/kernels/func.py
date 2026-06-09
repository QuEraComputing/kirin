# Kernels exercising the `function` dialect lowering: multi-argument functions,
# calls between kernels, nested calls, and recursion (expressed via if/else
# value-merge, since early `return` inside a branch is not supported).
#
# The call-graph shape mirrors `test/analysis/test_callgraph.py` on `main`.


def inc(x: int) -> int:
    return x + 1


def dec(x: int) -> int:
    return x - 1


def add3(a: int, b: int, c: int) -> int:
    return a + b + c


def combine(a: int, b: int) -> int:
    return inc(a) + dec(b)


def chained(x: int) -> int:
    return inc(inc(inc(x)))


def factorial(n: int) -> int:
    if n <= 1:
        r = 1
    else:
        r = n * factorial(n - 1)
    return r


def fib(n: int) -> int:
    if n < 2:
        r = n
    else:
        r = fib(n - 1) + fib(n - 2)
    return r
