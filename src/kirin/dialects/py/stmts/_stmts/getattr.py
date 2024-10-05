from kirin.decl import info, statement
from kirin.dialects.py.stmts.dialect import dialect
from kirin.ir import Pure, ResultValue, SSAValue, Statement


@statement(dialect=dialect)
class GetAttr(Statement):
    name = "getattr"
    traits = frozenset({Pure()})
    obj: SSAValue = info.argument(print=False)
    attrname: str = info.attribute(property=True)
    result: ResultValue = info.result()
