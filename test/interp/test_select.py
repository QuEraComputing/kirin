from dataclasses import dataclass

import pytest

from kirin.interp import MethodTable, StatementResult, impl
from kirin.lattice import EmptyLattice
from kirin.prelude import basic
from kirin.worklist import WorkList
from kirin.ir.method import Method
from kirin.dialects.py import stmts
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
    ) -> StatementResult[EmptyLattice]:
        return self.run_callable(method.code, (EmptyLattice(),) + args)


@stmts.dialect.register(key="test_interp")
class DialectMethodTable(MethodTable):

    @impl(stmts.NewTuple)
    def new_tuple(self, interp: DummyInterpreter, frame, stmt: stmts.NewTuple):
        return (EmptyLattice(),)


@basic
def main(x):
    return 1


def test_interp():
    interp = DummyInterpreter(basic)
    with pytest.raises(AttributeError):
        interp.eval(main, (EmptyLattice(),))
