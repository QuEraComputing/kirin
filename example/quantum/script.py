# type: ignore

# [section]
from enum import Enum
from typing import ClassVar
from dataclasses import dataclass
from qulacs import QuantumState


# this could be your own class implementing the runtime in whatever way you want
@dataclass
class Qubit:
    count: ClassVar[int] = 0  # class variable to count qubits
    id: int

    def __init__(self):
        self.id = Qubit.count
        Qubit.count += 1


# some your own classes
class Basis(Enum):
    X = "X"
    Y = "Y"
    Z = "Z"


# [section]
from kirin import ir, types, lowering
from kirin.decl import statement, info
from kirin.prelude import basic

# our language definitions and compiler begins
dialect = ir.Dialect("quantum")
QubitType = types.PyClass(Qubit)
StateType = types.PyClass(QuantumState)


@statement(dialect=dialect)
class NewQubit(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})
    state: ir.SSAValue = info.argument(
        StateType
    )  # we can use Python objects as arguments
    qubit: ir.ResultValue = info.result(QubitType)


@statement(dialect=dialect)
class X(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})
    state: ir.SSAValue = info.argument(StateType)
    qubit: ir.SSAValue = info.argument(QubitType)


@statement(dialect=dialect)
class H(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})
    state: ir.SSAValue = info.argument(StateType)
    qubit: ir.SSAValue = info.argument(QubitType)


@statement(dialect=dialect)
class CX(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})
    state: ir.SSAValue = info.argument(StateType)
    control: ir.SSAValue = info.argument(QubitType)
    target: ir.SSAValue = info.argument(QubitType)


@statement(dialect=dialect)
class CZ(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})
    state: ir.SSAValue = info.argument(StateType)
    control: ir.SSAValue = info.argument(QubitType)
    target: ir.SSAValue = info.argument(QubitType)


@statement(dialect=dialect)
class Measure(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})
    basis: Basis = (
        info.attribute()
    )  # we can use Python objects as attributes (compile-time values)!
    state: ir.SSAValue = info.argument(StateType)
    qubit: ir.SSAValue = info.argument(QubitType)
    result: ir.ResultValue = info.result(types.Int)


# now we have the miminim set of statements to represent a quantum circuit
# the following defines a group of "dialects" so we can use it as a decorator
@ir.dialect_group(basic.add(dialect))
def quantum(self):  # group self
    def run_default_pass(method, option_a=True):
        # default pass to run right after calling the decorator
        # a.k.a the default JIT compilation part of the compiler
        pass

    return run_default_pass


# Ok let's try it out
@quantum
def main(state: QuantumState):
    a = NewQubit(state)
    b = NewQubit(state)
    H(state, a)
    CX(state, control=a, target=b)
    return Measure(state, basis=Basis.Z, qubit=b)


# well Linter is mad at us

# [section]
# fortunately, Kirin provides a way to give hints to a standard Python linter
# now let's make some lowering wrappers to make Python type hinting happy


@lowering.wraps(NewQubit)
def new_qubit(state: QuantumState) -> Qubit: ...


@lowering.wraps(X)
def x(state: QuantumState, qubit: Qubit) -> None: ...


@lowering.wraps(H)
def h(state: QuantumState, qubit: Qubit) -> None: ...


@lowering.wraps(CX)
def cx(state: QuantumState, control: Qubit, target: Qubit) -> None: ...


@lowering.wraps(CZ)
def cz(state: QuantumState, control: Qubit, target: Qubit) -> None: ...


@lowering.wraps(Measure)
def measure(state: QuantumState, basis: Basis, qubit: Qubit) -> None: ...


# this is a lot nicer now!
@quantum
def main(state: QuantumState):
    a = new_qubit(state)
    b = new_qubit(state)
    h(state, a)
    h(state, b)
    cx(state, control=a, target=b)
    if measure(state, basis=Basis.Z, qubit=b):
        x(state, a)  # we can use the result of Measure to conditionally apply X gate
    return


main.print()

# Ok but this doesn't work yet, I cannot run it
# main()

# [section]
# we need to implement the runtime for the quantum circuit
# let's just import qulacs a quantum circuit simulator

from kirin import interp
from qulacs import gate, QuantumState


@dialect.register
class MethodTable(interp.MethodTable):
    @interp.impl(NewQubit)
    def impl_new_qubit(
        self, interp: interp.Interpreter, frame: interp.Frame, stmt: NewQubit
    ) -> tuple[Qubit]:
        return (Qubit(),)

    @interp.impl(X)
    def impl_x(self, interp: interp.Interpreter, frame: interp.Frame, stmt: X) -> None:
        state = frame.get_casted(
            stmt.state, QuantumState
        )  # assume state is QuantumState at runtime
        qubit = frame.get_casted(
            stmt.qubit, Qubit
        )  # we assume qubits are Qubit at runtime
        gate.X(qubit.id).update_quantum_state(state)

    @interp.impl(H)
    def impl_h(self, interp: interp.Interpreter, frame: interp.Frame, stmt: H) -> None:
        state = frame.get_casted(stmt.state, QuantumState)
        qubit = frame.get_casted(stmt.qubit, Qubit)
        gate.H(qubit.id).update_quantum_state(state)

    @interp.impl(CX)
    def impl_cx(
        self, interp: interp.Interpreter, frame: interp.Frame, stmt: CX
    ) -> None:
        state = frame.get_casted(stmt.state, QuantumState)
        control = frame.get_casted(stmt.control, Qubit)
        target = frame.get_casted(stmt.target, Qubit)
        print(f"Applying CNOT gate with control {control.id} and target {target.id}")
        gate.CNOT(control.id, target.id).update_quantum_state(state)

    @interp.impl(CZ)
    def impl_cz(
        self, interp: interp.Interpreter, frame: interp.Frame, stmt: CZ
    ) -> None:
        state = frame.get_casted(stmt.state, QuantumState)
        control = frame.get_casted(stmt.control, Qubit)
        target = frame.get_casted(stmt.target, Qubit)
        print(f"Applying CZ gate with control {control.id} and target {target.id}")
        gate.CZ(control.id, target.id).update_quantum_state(state)

    @interp.impl(Measure)
    def impl_measure(
        self, interp: interp.Interpreter, frame: interp.Frame, stmt: Measure
    ) -> tuple[int]:
        state = frame.get_casted(stmt.state, QuantumState)
        qubit = frame.get_casted(stmt.qubit, Qubit)
        basis = stmt.basis.value  # get the basis as a string
        result = gate.Measurement(qubit.id, qubit.id).update_quantum_state(state)
        return (
            state.get_classical_value(qubit.id),
        )  # return the measurement result as an int


state = QuantumState(2)  # 2 qubits
state.set_zero_state()
main(state)
print(state.get_vector())

# [section]
# ok now we can run it, how about rewriting the program?

from kirin.rewrite.abc import RewriteRule, RewriteResult


class CX2CZ(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, CX):
            return RewriteResult()

        H(node.state, node.target).insert_before(node)
        node.replace_by(
            cz_node := CZ(state=node.state, control=node.control, target=node.target)
        )
        H(node.state, node.target).insert_after(cz_node)
        return RewriteResult(has_done_something=True)


from kirin.rewrite import Walk

Walk(CX2CZ()).rewrite(main.code)
main.print()
