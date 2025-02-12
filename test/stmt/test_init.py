from typing import Any

from kirin import ir, types
from kirin.decl import info, statement

T = types.TypeVar("T")


@statement(repr=True)
class DummyStatement(ir.Statement):
    name = "constant"
    traits = frozenset({ir.Pure(), ir.ConstantLike()})

    # args
    noinfo: ir.SSAValue
    vararg_noinfo: tuple[ir.SSAValue, ...]
    xxx: ir.SSAValue = info.argument(T)
    xxx_Any: ir.SSAValue = info.argument()
    xxx_vararg: tuple[ir.SSAValue, ...] = info.argument()

    # results
    xxx_result: ir.ResultValue = info.result(T)

    # attributes
    xxx_property: Any = info.attribute(T, default="")
    xxx_attribute: ir.PyAttr[float] = info.attribute()
    xxx_dict: dict[str, int] = info.attribute()

    # regions
    xxx_region_noinfo: ir.Region = info.region()
    xxx_region: ir.Region = info.region(default_factory=ir.Region)
    xxx_region_multi: ir.Region = info.region(default_factory=ir.Region, multi=True)

    # blocks
    block_noinfo: ir.Block = info.block()
    block_default: ir.Block = info.block(default_factory=ir.Block)


def test_init():
    args = [ir.TestValue() for _ in range(8)]
    stmt = DummyStatement(
        args[0],
        (args[1], args[2]),
        args[3],
        xxx_Any=args[4],
        xxx_vararg=(args[5], args[6]),
        xxx_attribute=ir.PyAttr(2),
        xxx_property=1,
        xxx_dict={"a": 1},
        xxx_region_noinfo=ir.Region(),
        block_noinfo=ir.Block(),
    )

    print(stmt)

    assert stmt.noinfo is args[0]
    assert stmt.vararg_noinfo[0] is args[1]
    assert stmt.vararg_noinfo[1] is args[2]
    assert stmt.xxx is args[3]
    assert stmt.xxx_Any is args[4]
    assert stmt.xxx_vararg[0] is args[5]
    assert stmt.xxx_vararg[1] is args[6]

    assert isinstance(stmt.xxx_attribute, ir.PyAttr)
    assert stmt.xxx_attribute.data == 2
    assert stmt.xxx_property == 1
    assert isinstance(stmt.xxx_dict, dict)
    assert stmt.xxx_dict == {"a": 1}

    assert isinstance(stmt.xxx_region_noinfo, ir.Region)
    assert isinstance(stmt.xxx_region, ir.Region)
    assert isinstance(stmt.xxx_region_multi, ir.Region)
    assert isinstance(stmt.block_noinfo, ir.Block)
    assert isinstance(stmt.block_default, ir.Block)
