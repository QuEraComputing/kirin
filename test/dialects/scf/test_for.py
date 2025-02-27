import pytest

from kirin import ir, types
from kirin.prelude import python_basic
from kirin.dialects import py, scf, func, ilist, lowering
from kirin.exceptions import VerificationError


def test_cons():
    x0 = py.Constant(0)
    iter = py.Constant(range(5))
    body = ir.Region(ir.Block([]))
    idx = body.blocks[0].args.append_from(types.Any, "idx")
    body.blocks[0].args.append_from(types.Any, "acc")
    body.blocks[0].stmts.append(scf.Yield(idx))
    stmt = scf.For(iter.result, body, x0.result)
    assert len(stmt.results) == 1

    body = ir.Region(ir.Block([]))
    idx = body.blocks[0].args.append_from(types.Any, "idx")
    body.blocks[0].stmts.append(scf.Yield(idx))

    with pytest.raises(VerificationError):
        stmt = scf.For(iter.result, body, x0.result)
        stmt.verify()

    body = ir.Region(ir.Block([]))
    idx = body.blocks[0].args.append_from(types.Any, "idx")
    with pytest.raises(VerificationError):
        stmt = scf.For(iter.result, body, x0.result)
        stmt.verify()


def test_exec():
    xs = ilist.IList([(1, 2), (3, 4)])

    @python_basic.union(
        [func, scf, py.unpack, ilist, lowering.func, lowering.range.ilist]
    )
    def main(x):
        for a, b in xs:
            x = x + a
        return x

    main.print()
    assert main(0) == 4


def test_issue_213():

    @python_basic.union(
        [func, scf, py.unpack, ilist, lowering.func, lowering.range.ilist]
    )
    def main():
        j = 0.0
        i = 0
        for k in range(2):
            j = j + i + k

        for k in range(2):
            j = j + i

        return j

    assert main.py_func is not None
    assert main() == main.py_func()
