from typing import Any

from kirin import ir, types
from kirin.decl import info, fields, statement
from kirin.decl.info import (
    BlockField,
    RegionField,
    ResultField,
    ArgumentField,
    AttributeField,
)

T = types.TypeVar("T")


@statement
class DummyStatement(ir.Statement):
    name = "constant"
    traits = frozenset({ir.Pure(), ir.ConstantLike()})

    # args
    noinfo: ir.SSAValue
    vararg_noinfo: tuple[ir.SSAValue, ...]
    xxx: ir.SSAValue = info.argument(T)
    xxx_Any: ir.SSAValue = info.argument()
    # xxx_vararg: list[ir.SSAValue] = argument()

    # results
    xxx_result: ir.ResultValue = info.result(T)

    # attributes
    xxx_attribute: Any = info.attribute(T)
    xxx_dict: dict[str, int] = info.attribute()

    # regions
    xxx_region_noinfo: ir.Region = info.region()
    xxx_region: ir.Region = info.region(default_factory=ir.Region)
    xxx_region_multi: ir.Region = info.region(default_factory=ir.Region, multi=True)

    # blocks
    block_noinfo: ir.Block = info.block()
    block_default: ir.Block = info.block(default_factory=ir.Block)


ff = fields(DummyStatement)


def test_scan_fields():
    noinfo = ff.args["noinfo"]
    assert isinstance(noinfo, ArgumentField)
    assert noinfo.type == types.Any
    assert noinfo.group is False

    vararg_noinfo = ff.args["vararg_noinfo"]
    assert isinstance(vararg_noinfo, ArgumentField)
    assert vararg_noinfo.type == types.Any
    assert vararg_noinfo.group is True

    xxx = ff.args["xxx"]
    assert isinstance(xxx, ArgumentField)
    assert xxx.type == T
    assert xxx.group is False

    xxx_Any = ff.args["xxx_Any"]
    assert isinstance(xxx_Any, ArgumentField)
    assert xxx_Any.type == types.Any
    assert xxx_Any.group is False

    xxx_result = ff.results["xxx_result"]
    assert isinstance(xxx_result, ResultField)
    assert xxx_result.type == T

    xxx_attribute = ff.attributes["xxx_attribute"]
    assert isinstance(xxx_attribute, AttributeField)
    assert xxx_attribute.type == T

    xxx_dict = ff.attributes["xxx_dict"]
    assert isinstance(xxx_dict, AttributeField)
    assert xxx_dict.type == types.Dict[types.String, types.Int]

    xxx_region_noinfo = ff.regions["xxx_region_noinfo"]
    assert isinstance(xxx_region_noinfo, RegionField)
    assert xxx_region_noinfo.multi is False

    xxx_region = ff.regions["xxx_region"]
    assert isinstance(xxx_region, RegionField)
    assert xxx_region.multi is False

    xxx_region_multi = ff.regions["xxx_region_multi"]
    assert isinstance(xxx_region_multi, RegionField)
    assert xxx_region_multi.multi is True

    block_noinfo = ff.blocks["block_noinfo"]
    assert isinstance(block_noinfo, BlockField)

    block = ff.blocks["block_default"]
    assert isinstance(block, BlockField)
