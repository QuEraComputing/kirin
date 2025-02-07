from kirin import ir
from kirin.prelude import python_basic
from kirin.dialects import scf, func, lowering

# TODO:
# test_cons
# cond = py.Constant(True)
# then_body = ir.Region(ir.Block())
# else_body = ir.Region(ir.Block())

# ifelse = scf.IfElse(cond.result, ir.Block([scf.Yield(cond.result)]), ir.Block([scf.Yield(cond.result)]))
# ifelse.print()

# ir.Block([ifelse]).print()


@ir.dialect_group(python_basic.union([func, scf, lowering.func]))
def kernel(self):
    def run_pass(method):
        pass

    return run_pass


@kernel
def main(x):
    if x > 0:
        y = x + 1
        z = y + 1
        return z
    else:
        y = x + 2
        z = y + 2

    if x < 0:
        y = y + 3
        z = y + 3
    else:
        y = x + 4
        z = y + 4
    return y, z


main.print()
# print(main(1))
