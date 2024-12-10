from dataclasses import dataclass

import pytest

from kirin.analysis.dataflow.forward import ForwardExtra
from kirin.dialects.py import stmts
from kirin.interp import DialectInterpreter, ResultValue, impl
from kirin.interp.base import InterpResult
from kirin.ir.method import Method
from kirin.ir.nodes.region import Region
from kirin.lattice import EmptyLattice
from kirin.prelude import basic
from kirin.worklist import WorkList


@dataclass(init=False)
class DummyInterpreter(ForwardExtra[EmptyLattice, None]):
    keys = ["test_interp", "main", "empty"]
    lattice = EmptyLattice

    @classmethod
    def default_worklist(cls) -> WorkList:
        return WorkList()

    def run_method_region(
        self, mt: Method, body: Region, args: tuple[EmptyLattice, ...]
    ) -> InterpResult[EmptyLattice]:
        return self.run_ssacfg_region(body, (EmptyLattice(),) + args)


@stmts.dialect.register(key="test_interp")
class DialectInterp(DialectInterpreter):

    @impl(stmts.NewTuple)
    def new_tuple(
        self, interp: DummyInterpreter, stmt: stmts.NewTuple, values: tuple
    ) -> ResultValue:
        return ResultValue(EmptyLattice())


@basic
def main(x):
    return 1


def test_interp():
    interp = DummyInterpreter(basic)
    with pytest.raises(AttributeError):
        interp.eval(main, (EmptyLattice(),))
