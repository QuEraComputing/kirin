from kirin import ir
from kirin.passes import Fold
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


def test_basic_if_else():
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
    print(main(1))


def test_if_else_defs():

    @kernel
    def main(n: int):
        x = 0

        if x == n:
            x = 1
        else:
            y = 2  # noqa: F841

        return x

    main.print()

    # make sure fold doesn't remove the nested def
    main2 = main.similar(kernel)
    Fold(main2.dialects)(main2)

    assert main(0) == 1 == main2(0)
    assert main(10) == 0 == main2(4)

    main2.print()

    @kernel
    def main_elif(n: int):
        x = 0

        if x == n:
            x = 3
        elif x == n + 1:
            x = 4

        return x

    main_elif.print()

    main_elif2 = main_elif.similar(kernel)
    Fold(main_elif2.dialects)(main_elif2)

    main_elif2.print()

    assert main_elif(0) == 3 == main_elif2(0)
    assert main_elif(-1) == 4 == main_elif2(-1)
    assert main_elif(5) == 0 == main_elif2(7)

    @kernel
    def main_nested_if(n: int):
        x = 0

        if n > 0:
            if n > 1:
                if n == 3:
                    x = 4

        return x

    main_nested_if.print()

    main_nested_if2 = main_nested_if.similar(kernel)
    Fold(main_nested_if2.dialects)(main_nested_if2)

    assert main_nested_if(3) == 4 == main_nested_if2(3)
    assert main_nested_if(10) == 0 == main_nested_if2(8)
