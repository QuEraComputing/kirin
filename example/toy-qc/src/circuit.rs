use crate::types::QubitType;
use kirin::prelude::*;

/// Function body holding a DiGraph for circuit-stage programs.
/// Circuits are naturally directed acyclic graphs: qubit values flow
/// forward through gates.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "{body}")]
pub struct CircuitFunction {
    pub body: DiGraph,
}

/// Single-qubit Hadamard gate.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "$h {qubit} -> {result:type}")]
pub struct H {
    pub qubit: SSAValue,
    pub result: ResultValue,
}

/// Two-qubit CNOT gate with two results (control out, target out).
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "$cnot {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}")]
pub struct Cnot {
    pub ctrl: SSAValue,
    pub tgt: SSAValue,
    pub ctrl_out: ResultValue,
    pub tgt_out: ResultValue,
}

/// Z-rotation gate with angle parameter.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "$rz({angle}) {qubit} -> {result:type}")]
pub struct Rz {
    pub angle: f64,
    pub qubit: SSAValue,
    pub result: ResultValue,
}

/// Measurement gate.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "$measure {qubit} -> {result:type}")]
pub struct Measure {
    pub qubit: SSAValue,
    pub result: ResultValue,
}

/// Circuit dialect language enum.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
pub enum Circuit {
    #[wraps]
    H(H),
    #[wraps]
    Cnot(Cnot),
    #[wraps]
    Rz(Rz),
    #[wraps]
    Measure(Measure),
    #[wraps]
    CircuitFunction(CircuitFunction),
}
