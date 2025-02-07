from kirin.prelude import basic_no_opt
from kirin.lowering import Lowering

lowering = Lowering(basic_no_opt)
range_a = range(10)


def simple_loop(x):
    for i in range(10):
        for j in range(10):
            x = x + i + j


code = lowering.run(simple_loop)
# code.print()


def branch_pass():
    if True:
        pass
    else:
        pass


code = lowering.run(branch_pass, compactify=False)
