from typing import Any

import pytest

from kirin import ir, types
from kirin.decl import info, statement
from kirin.prelude import python_no_opt
from kirin.dialects import func
from kirin.lowering import Lowering
from kirin.exceptions import DialectLoweringError

T = types.TypeVar("T")

dialect = ir.Dialect("test")


@statement(dialect=dialect, repr=True)
class DummyStatement(ir.Statement):
    name = "dummy"
    traits = frozenset({ir.Pure(), ir.ConstantLike(), ir.FromPythonCall()})

    # args
    noinfo: ir.SSAValue
    vararg_noinfo: tuple[ir.SSAValue, ...]
    xxx: ir.SSAValue = info.argument(T)
    xxx_Any: ir.SSAValue = info.argument()
    xxx_vararg: tuple[ir.SSAValue, ...] = info.argument()

    # results
    xxx_result: ir.ResultValue = info.result(T)

    # attributes
    xxx_property: Any = info.attribute(default="")
    xxx_attribute: bool = info.attribute()


def dummy(
    noinfo,
    vararg_noinfo,
    xxx,
    xxx_Any,
    xxx_vararg,
):
    return DummyStatement(
        noinfo,
        (vararg_noinfo,),
        xxx,
        xxx_Any,
        xxx_vararg=(xxx_vararg,),
        xxx_property="xxx_property",
        xxx_attribute=False,
    )


def dummy_2(
    noinfo,
    vararg_noinfo,
    xxx,
    xxx_Any,
    xxx_vararg,
):
    return DummyStatement(
        noinfo,
        (vararg_noinfo,),
        xxx,
        xxx_Any=xxx_Any,
        xxx_vararg=(xxx_vararg,),
        xxx_property="xxx_property",
        xxx_attribute=False,
    )


def non_const(
    noinfo,
    vararg_noinfo,
    xxx,
    xxx_Any,
    xxx_vararg,
    non_const,
):
    return DummyStatement(
        noinfo,
        (vararg_noinfo,),
        xxx,
        xxx_Any=xxx_Any,
        xxx_vararg=(xxx_vararg,),
        xxx_property=non_const,
        xxx_attribute=False,
    )


def no_group(
    noinfo,
    vararg_noinfo,
    xxx,
    xxx_Any,
    xxx_vararg,
):
    return DummyStatement(
        noinfo,
        vararg_noinfo,
        xxx,
        xxx_Any=xxx_Any,
        xxx_vararg=(xxx_vararg,),
        xxx_property="non_const",
        xxx_attribute=False,
    )


def test_from_python_call():
    assert DummyStatement.dialect is dialect
    lowering = Lowering(python_no_opt.data.union([func, dialect]))

    code: func.Function = lowering.run(dummy)  # type: ignore
    block = code.body.blocks[0]
    stmt: DummyStatement = block.stmts.at(0)  # type: ignore
    assert stmt.noinfo is block.args[1]
    assert stmt.vararg_noinfo == (block.args[2],)
    assert stmt.xxx is block.args[3]
    assert stmt.xxx_Any is block.args[4]
    assert stmt.xxx_vararg == (block.args[5],)
    assert stmt.xxx_result.type == T
    assert stmt.xxx_property == "xxx_property"
    assert stmt.xxx_attribute is False  # type: ignore

    code: func.Function = lowering.run(dummy_2)  # type: ignore
    block = code.body.blocks[0]
    stmt: DummyStatement = block.stmts.at(0)  # type: ignore
    assert stmt.noinfo is block.args[1]
    assert stmt.vararg_noinfo == (block.args[2],)
    assert stmt.xxx is block.args[3]
    assert stmt.xxx_Any is block.args[4]
    assert stmt.xxx_vararg == (block.args[5],)
    assert stmt.xxx_result.type == T
    assert stmt.xxx_property == "xxx_property"
    assert stmt.xxx_attribute is False  # type: ignore

    with pytest.raises(DialectLoweringError):
        lowering.run(non_const)

    with pytest.raises(DialectLoweringError):
        lowering.run(no_group)
