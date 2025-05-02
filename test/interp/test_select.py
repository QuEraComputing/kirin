from dataclasses import dataclass

import pytest

from kirin import interp
from kirin.lattice import EmptyLattice
from kirin.prelude import basic
from kirin.dialects import py
from kirin.ir.method import Method
from kirin.ir.nodes.stmt import Statement
from kirin.analysis.forward import Forward, ForwardFrame


@dataclass
class DummyInterpreter(Forward[EmptyLattice]):
    keys = ("test_interp",)
    lattice = EmptyLattice

    def method_self(self, method: Method) -> EmptyLattice:
        return EmptyLattice()

    def eval_fallback(
        self, frame: ForwardFrame[EmptyLattice], node: Statement
    ) -> interp.StatementResult[EmptyLattice]:
        ret = super().eval_fallback(frame, node)
        print("fallback: ", ret)
        return ret


@py.tuple.dialect.register(key="test_interp")
class DialectMethodTable(interp.MethodTable):

    @interp.impl(py.tuple.New)
    def new_tuple(self, interp: DummyInterpreter, frame, stmt: py.tuple.New):
        return (EmptyLattice(),)


@basic
def main(x):
    return 1


def test_interp():
    interp_ = DummyInterpreter(basic)
    with pytest.raises(NotImplementedError):
        interp_.run(main, EmptyLattice())

    interp_ = DummyInterpreter(basic)
    interp_.run_no_raise(main, EmptyLattice())
