//! Type-level profiles for the toy-lang interpreter.
//!
//! Each profile binds the interpreter's `Stage`, `Value`, `Frame`, etc., into
//! a single marker type so call sites can write
//! `ConcreteInterpreter::<ToySourceConcrete>::new(pipeline)` instead of a
//! six-parameter ascription.

use kirin_constprop::{ConstPropOwner, ConstPropSummary};
use kirin_interpreter::{AbstractBlockTransfer, FixpointProfile, InterpreterProfile};
use kirin_scf::ScfForConstPropSummary;

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

use super::{ConstProp, ToyCompletion, ToyError, ToyFrame, ToyStageFrame};

/// Concrete execution at the source (HighLevel) stage with `i64` values.
pub struct ToySourceConcrete;

impl InterpreterProfile for ToySourceConcrete {
    type Stage = Stage;
    type Value = i64;
    type Frame = ToyFrame<HighLevel, i64>;
    type Completion = ToyCompletion<i64>;
    type Error = ToyError;
}

/// Concrete execution at the lowered stage with `i64` values.
pub struct ToyLoweredConcrete;

impl InterpreterProfile for ToyLoweredConcrete {
    type Stage = Stage;
    type Value = i64;
    type Frame = ToyFrame<LowLevel, i64>;
    type Completion = ToyCompletion<i64>;
    type Error = ToyError;
}

/// Abstract const-prop execution at the source (HighLevel) stage (single run).
#[cfg(test)]
pub struct ToySourceConstProp;

#[cfg(test)]
impl InterpreterProfile for ToySourceConstProp {
    type Stage = Stage;
    type Value = ConstProp;
    type Frame = ToyFrame<HighLevel, ConstProp, AbstractBlockTransfer<ConstProp>>;
    type Completion = ToyCompletion<ConstProp>;
    type Error = ToyError;
}

/// Abstract const-prop execution at the lowered stage (single run).
#[cfg(test)]
pub struct ToyLoweredConstProp;

#[cfg(test)]
impl InterpreterProfile for ToyLoweredConstProp {
    type Stage = Stage;
    type Value = ConstProp;
    type Frame = ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>;
    type Completion = ToyCompletion<ConstProp>;
    type Error = ToyError;
}

/// Const-prop fixpoint analysis that traverses both stages via [`ToyStageFrame`].
pub struct ToyConstPropFunction;

impl InterpreterProfile for ToyConstPropFunction {
    type Stage = Stage;
    type Value = ConstProp;
    type Frame = ToyStageFrame<ConstProp, AbstractBlockTransfer<ConstProp>>;
    type Completion = ToyCompletion<ConstProp>;
    type Error = ToyError;
}

impl FixpointProfile for ToyConstPropFunction {
    type SummaryKey = ConstPropOwner;
    type Summary = ConstPropSummary<ConstProp, ScfForConstPropSummary<ConstProp>>;
}

/// Const-prop fixpoint analysis pinned to the lowered stage.
#[cfg(test)]
pub struct ToyConstPropLowered;

#[cfg(test)]
impl InterpreterProfile for ToyConstPropLowered {
    type Stage = Stage;
    type Value = ConstProp;
    type Frame = ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>;
    type Completion = ToyCompletion<ConstProp>;
    type Error = ToyError;
}

#[cfg(test)]
impl FixpointProfile for ToyConstPropLowered {
    type SummaryKey = ConstPropOwner;
    type Summary = ConstPropSummary<ConstProp, ScfForConstPropSummary<ConstProp>>;
}
