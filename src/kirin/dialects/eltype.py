"""this dialect offers a statement `eltype` for other dialects'
type inference to query/implement the element type of a value.
For example, the `ilist` dialect implements the `eltype` statement
on the `ilist.IList` type to return the element type.
"""

from kirin import ir
from kirin.decl import info, statement

dialect = ir.Dialect("eltype")


@statement(dialect=dialect)
class ElType(ir.Statement):
    """Returns the element type of a value.

    This statement is used by other dialects to query the element type of a value.
    """

    container: ir.SSAValue = info.argument(ir.types.PyClass(ir.types.TypeAttribute))
    """The value to query the element type of."""
    elem: ir.ResultValue = info.result(ir.types.PyClass(ir.types.TypeAttribute))
    """The element type of the value."""
