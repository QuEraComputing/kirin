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
#[chumsky(format = "{result:name} = {.h} {qubit} -> {result:type}")]
pub struct H {
    pub qubit: SSAValue,
    pub result: ResultValue,
}

/// Two-qubit CNOT gate — control output.
/// CNOT is split into two operations because the derive does not support
/// multi-result format strings reliably.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "{ctrl_out:name} = {.cnot_ctrl} {ctrl}, {tgt} -> {ctrl_out:type}")]
pub struct CnotCtrl {
    pub ctrl: SSAValue,
    pub tgt: SSAValue,
    pub ctrl_out: ResultValue,
}

/// Two-qubit CNOT gate — target output.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "{tgt_out:name} = {.cnot_tgt} {ctrl}, {tgt} -> {tgt_out:type}")]
pub struct CnotTgt {
    pub ctrl: SSAValue,
    pub tgt: SSAValue,
    pub tgt_out: ResultValue,
}

/// Z-rotation gate with angle parameter.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "{result:name} = {.rz}({angle}) {qubit} -> {result:type}")]
pub struct Rz {
    pub angle: f64,
    pub qubit: SSAValue,
    pub result: ResultValue,
}

/// Measurement gate.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "{result:name} = {.measure} {qubit} -> {result:type}")]
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
    CnotCtrl(CnotCtrl),
    #[wraps]
    CnotTgt(CnotTgt),
    #[wraps]
    Rz(Rz),
    #[wraps]
    Measure(Measure),
    #[wraps]
    CircuitFunction(CircuitFunction),
}
