import pytest

from kirin import ir
from kirin.prelude import python_basic
from kirin.dialects import py, scf, func, ilist
from kirin.exceptions import DialectLoweringError


def test_cons():
    x0 = py.Constant(0)
    iter = py.Constant(range(5))
    body = ir.Region(ir.Block([]))
    idx = body.blocks[0].args.append_from(ir.types.Any, "idx")
    body.blocks[0].args.append_from(ir.types.Any, "acc")
    body.blocks[0].stmts.append(scf.Yield(idx))
    stmt = scf.For(iter.result, body, x0.result)
    assert len(stmt.results) == 1

    body = ir.Region(ir.Block([]))
    idx = body.blocks[0].args.append_from(ir.types.Any, "idx")
    body.blocks[0].stmts.append(scf.Yield(idx))

    with pytest.raises(DialectLoweringError):
        stmt = scf.For(iter.result, body, x0.result)

    body = ir.Region(ir.Block([]))
    idx = body.blocks[0].args.append_from(ir.types.Any, "idx")
    with pytest.raises(DialectLoweringError):
        stmt = scf.For(iter.result, body, x0.result)


def test_exec():
    xs = ilist.IList([(1, 2), (3, 4)])

    @python_basic.union([func, scf, py.range, py.unpack, ilist])
    def main(x):
        for a, b in xs:
            x = x + a
        return x

    main.print()
    assert main(0) == 4
