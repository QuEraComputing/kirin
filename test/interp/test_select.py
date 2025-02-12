from dataclasses import dataclass

import pytest

from kirin import interp
from kirin.lattice import EmptyLattice
from kirin.prelude import basic
from kirin.dialects import py
from kirin.ir.method import Method
from kirin.ir.nodes.stmt import Statement
from kirin.analysis.forward import Forward, ForwardFrame


@dataclass(init=False)
class DummyInterpreter(Forward[EmptyLattice]):
    keys = ["test_interp"]
    lattice = EmptyLattice

    def run_method(self, method: Method, args: tuple[EmptyLattice, ...]):
        return self.run_callable(method.code, (EmptyLattice(),) + args)

    def eval_stmt_fallback(
        self, frame: ForwardFrame[EmptyLattice], stmt: Statement
    ) -> tuple[EmptyLattice, ...] | interp.SpecialValue[EmptyLattice]:
        ret = super().eval_stmt_fallback(frame, stmt)
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
    with pytest.raises(interp.InterpreterError):
        interp_.run(main, (EmptyLattice(),)).expect()
