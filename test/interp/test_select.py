from dataclasses import dataclass

import pytest

from kirin.interp import MethodTable, impl
from kirin.lattice import EmptyLattice
from kirin.prelude import basic
from kirin.worklist import WorkList
from kirin.ir.method import Method
from kirin.dialects.py import stmts
from kirin.ir.nodes.region import Region
from kirin.analysis.forward import ForwardExtra


@dataclass(init=False)
class DummyInterpreter(ForwardExtra[EmptyLattice, None]):
    keys = ["test_interp", "main", "empty"]
    lattice = EmptyLattice

    @classmethod
    def default_worklist(cls) -> WorkList:
        return WorkList()

    def run_method_region(
        self, mt: Method, body: Region, args: tuple[EmptyLattice, ...]
    ) -> EmptyLattice:
        return self.run_ssacfg_region(body, (EmptyLattice(),) + args)


@stmts.dialect.register(key="test_interp")
class DialectInterp(MethodTable):

    @impl(stmts.NewTuple)
    def new_tuple(self, interp: DummyInterpreter, stmt: stmts.NewTuple, values: tuple):
        return (EmptyLattice(),)


@basic
def main(x):
    return 1


def test_interp():
    interp = DummyInterpreter(basic)
    with pytest.raises(AttributeError):
        interp.eval(main, (EmptyLattice(),))
