from numbers import Number

import numpy as np

from kirin import ir, types, lowering
from kirin.decl import info, statement

from .dialect import dialect_


@statement
class PauliOperator(ir.Statement):
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    pre_factor: Number = info.attribute(default=1)
    result: ir.ResultValue = info.result(types.PyClass(np.matrix))


@statement(dialect=dialect_)
class X(PauliOperator):
    pass


@statement(dialect=dialect_)
class Y(PauliOperator):
    pass


@statement(dialect=dialect_)
class Z(PauliOperator):
    pass


@statement(dialect=dialect_)
class Id(PauliOperator):
    pass
