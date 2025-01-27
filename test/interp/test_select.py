from dataclasses import dataclass

import pytest

from kirin import interp
from kirin.lattice import EmptyLattice
from kirin.prelude import basic
from kirin.dialects import py
from kirin.worklist import WorkList
from kirin.ir.method import Method
from kirin.analysis.forward import ForwardExtra


@dataclass(init=False)
class DummyInterpreter(ForwardExtra[EmptyLattice, None]):
    keys = ["test_interp", "main", "empty"]
    lattice = EmptyLattice

    @classmethod
    def default_worklist(cls) -> WorkList:
        return WorkList()

    def run_method(
        self, method: Method, args: tuple[EmptyLattice, ...]
    ) -> EmptyLattice:
        return self.run_callable(method.code, (EmptyLattice(),) + args)


@py.tuple.dialect.register(key="test_interp")
class DialectMethodTable(interp.MethodTable):

    @interp.impl(py.tuple.New)
    def new_tuple(self, interp: DummyInterpreter, frame, stmt: py.tuple.New):
        return (EmptyLattice(),)


@basic
def main(x):
    return 1


def test_interp():
    interp = DummyInterpreter(basic)
    with pytest.raises(AttributeError):
        interp.run_analysis(main)
