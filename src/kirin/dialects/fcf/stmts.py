from kirin import ir
from kirin.decl import info, statement
from kirin.dialects.fcf.dialect import dialect
from kirin.dialects.py import types


@statement(dialect=dialect)
class Foldl(ir.Statement):
    fn: ir.SSAValue = info.argument(types.PyClass(ir.Method))
    coll: ir.SSAValue = info.argument(types.Any)  # TODO: make this more precise
    init: ir.SSAValue = info.argument(types.Any)
    result: ir.ResultValue = info.result(types.Any)


@statement(dialect=dialect)
class Foldr(ir.Statement):
    fn: ir.SSAValue = info.argument(types.PyClass(ir.Method))
    coll: ir.SSAValue = info.argument(types.Any)
    init: ir.SSAValue = info.argument(types.Any)
    result: ir.ResultValue = info.result(types.Any)


@statement(dialect=dialect)
class MapList(ir.Statement):
    fn: ir.SSAValue = info.argument(types.PyClass(ir.Method))
    coll: ir.SSAValue = info.argument(types.List)
    result: ir.ResultValue = info.result(types.List)


@statement(dialect=dialect)
class Scan(ir.Statement):
    fn: ir.SSAValue = info.argument(types.PyClass(ir.Method))
    init: ir.SSAValue = info.argument(types.Any)
    coll: ir.SSAValue = info.argument(types.List)
    result: ir.ResultValue = info.result(types.Tuple[types.Any, types.List])
