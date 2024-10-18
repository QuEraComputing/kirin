from dataclasses import dataclass

import pytest

from kirin.analysis.dataflow.forward import ForwardDataFlowAnalysis
from kirin.dialects.py import stmts
from kirin.interp import DialectInterpreter, ResultValue, impl
from kirin.lattice import EmptyLattice
from kirin.prelude import basic
from kirin.worklist import WorkList


@dataclass(init=False)
class Interpreter(ForwardDataFlowAnalysis[EmptyLattice, WorkList]):
    keys = ["test_interp", "main", "empty"]

    @classmethod
    def bottom_value(cls) -> EmptyLattice:
        return EmptyLattice()

    @classmethod
    def default_worklist(cls) -> WorkList:
        return WorkList()


@stmts.dialect.register(key="test_interp")
class DialectInterp(DialectInterpreter):

    @impl(stmts.NewTuple)
    def new_tuple(
        self, interp: Interpreter, stmt: stmts.NewTuple, values: tuple
    ) -> ResultValue:
        return ResultValue(EmptyLattice())


@basic
def main(x):
    return 1


def test_interp():
    interp = Interpreter(basic)
    with pytest.raises(AttributeError):
        interp.eval(main, (EmptyLattice(),))
