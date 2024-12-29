from typing import Any

from kirin import ir
from kirin.decl import info, fields, statement
from kirin.dialects.py import types

T = types.PyTypeVar("T")


@statement(repr=True)
class TestStatement(ir.Statement):
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
    xxx_property: Any = info.attribute(T, default="", property=True)
    xxx_attribute: Any = info.attribute(T)
    xxx_dict: dict[str, int] = info.attribute()

    # regions
    xxx_region_noinfo: ir.Region = info.region()
    xxx_region: ir.Region = info.region(default_factory=ir.Region)
    xxx_region_multi: ir.Region = info.region(default_factory=ir.Region, multi=True)

    # blocks
    block_noinfo: ir.Block = info.block()
    block_default: ir.Block = info.block(default_factory=ir.Block)


fields(TestStatement)

print(TestStatement._arg_groups)
stmt = TestStatement(
    ir.TestValue(),
    (ir.TestValue(), ir.TestValue()),
    ir.TestValue(),
    xxx_Any=ir.TestValue(),
    xxx_vararg=(ir.TestValue(), ir.TestValue()),
    xxx_attribute=2,
    xxx_property=1,
    xxx_dict={"a": 1},
    xxx_region_noinfo=ir.Region(),
    block_noinfo=ir.Block(),
)

print(stmt)
# print(stmt.xxx_property)
# print(stmt.properties)
# print(stmt.xxx_Any)
# print(stmt.xxx_vararg)
# print(stmt.block_noinfo)
# print(stmt.xxx_region_multi)
# print(stmt.xxx_property)
# stmt.print()
# print(stmt.__repr__())

# ff = fields(TestStatement)
# from rich import print

# print(ff.attributes["xxx_dict"])
# # print(ff.args)
# # print(ff.results)
# # print(ff.attributes)
# # print(ff.regions)
