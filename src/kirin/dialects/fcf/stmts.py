from kirin import ir
from kirin.decl import info, statement
from kirin.dialects.fcf.dialect import dialect


@statement(dialect=dialect)
class Foldl(ir.Statement):
    fn: ir.SSAValue = info.argument(ir.types.PyClass(ir.Method))
    coll: ir.SSAValue = info.argument(ir.types.Any)  # TODO: make this more precise
    init: ir.SSAValue = info.argument(ir.types.Any)
    result: ir.ResultValue = info.result(ir.types.Any)


@statement(dialect=dialect)
class Foldr(ir.Statement):
    fn: ir.SSAValue = info.argument(ir.types.PyClass(ir.Method))
    coll: ir.SSAValue = info.argument(ir.types.Any)
    init: ir.SSAValue = info.argument(ir.types.Any)
    result: ir.ResultValue = info.result(ir.types.Any)


@statement(dialect=dialect)
class Map(ir.Statement):
    fn: ir.SSAValue = info.argument(ir.types.PyClass(ir.Method))
    coll: ir.SSAValue = info.argument(ir.types.Any)
    result: ir.ResultValue = info.result(ir.types.List)


@statement(dialect=dialect)
class Scan(ir.Statement):
    fn: ir.SSAValue = info.argument(ir.types.PyClass(ir.Method))
    init: ir.SSAValue = info.argument(ir.types.Any)
    coll: ir.SSAValue = info.argument(ir.types.List)
    result: ir.ResultValue = info.result(ir.types.Tuple[ir.types.Any, ir.types.List])
