"""A minimal language example with a single pass that does nothing."""

from kirin import ir
from kirin.dialects import cf, py, func, lowering


@ir.dialect_group(
    [
        func,
        lowering.func,
        lowering.call,
        lowering.cf,
        py.base,
        py.constant,
        py.assign,
        py.binop,
        py.unary,
    ]
)
def simple(self):
    def run_pass(mt):
        return mt

    return run_pass


@simple
def main(x):
    y = x + 1
    return y


main.print()


@simple.add(cf).add(py.cmp)
def main2(x):
    y = x + 1
    if y > 0:  # errors
        return y
    else:
        return -y


main2.print()
