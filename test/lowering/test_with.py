from kirin import ir
from kirin.decl import info, statement
from kirin.prelude import python_no_opt
from kirin.dialects import cf, py, func
from kirin.lowering import Lowering

dialect = ir.Dialect("test")


@statement(dialect=dialect)
class Adjoint(ir.Statement):
    traits = frozenset({ir.FromPythonWithSingleItem()})
    body: ir.Region = info.region()
    result: ir.ResultValue = info.result()


def with_example(x):
    y = 1
    with Adjoint() as f:  # type: ignore
        y = x + 1
    return y, f


def test_with_lowering():
    lowering = Lowering(python_no_opt.union([cf, func, dialect]))
    code = lowering.run(with_example)
    code.print()
    assert isinstance(code, func.Function)
    stmts = code.body.blocks[0].stmts
    assert isinstance(stmts.at(0), py.Constant)
    adjoint = stmts.at(1)
    assert isinstance(adjoint, Adjoint)
    assert len(adjoint.body.blocks) == 1
    add = adjoint.body.blocks[0].stmts.at(1)
    assert isinstance(add, py.Add)
    assert isinstance(add.lhs, ir.BlockArgument)
    assert isinstance(add.rhs, ir.SSAValue)
    assert adjoint.result.name == "f"
