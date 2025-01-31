from kirin import ir
from kirin.dialects import py, scf, func
from kirin.prelude import python_basic

x0 = py.Constant(0)
iter = py.Constant(range(5))
body = ir.Region(ir.Block([]))
idx = body.blocks[0].args.append_from(ir.types.Any, 'idx')
acc = body.blocks[0].args.append_from(ir.types.Any, 'acc')
body.blocks[0].stmts.append(scf.Yield(idx))
stmt = scf.For(iter.result, body, x0.result)
ir.Block([stmt]).print()


@python_basic.union([func, scf, py.range])
def main(x):
    for i in range(5):
        x = x + i
    return x

main.print()
