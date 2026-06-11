//! Test-only interpreter profiles.

use kirin_constprop::{ConstPropOwner, ConstPropSummary};
use kirin_interpreter::{AbstractBlockTransfer, FixpointProfile, InterpreterProfile};
use kirin_scf::ScfForConstPropSummary;

use crate::interpreter::{ConstProp, ToyCompletion, ToyError, ToyFrame};
use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

/// Abstract const-prop execution at the source (HighLevel) stage (single run).
pub(super) struct ToySourceConstProp;

impl InterpreterProfile for ToySourceConstProp {
    type Stage = Stage;
    type Value = ConstProp;
    type Frame = ToyFrame<HighLevel, ConstProp, AbstractBlockTransfer<ConstProp>>;
    type Completion = ToyCompletion<ConstProp>;
    type Error = ToyError;
}

/// Abstract const-prop execution at the lowered stage (single run).
pub(super) struct ToyLoweredConstProp;

impl InterpreterProfile for ToyLoweredConstProp {
    type Stage = Stage;
    type Value = ConstProp;
    type Frame = ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>;
    type Completion = ToyCompletion<ConstProp>;
    type Error = ToyError;
}

/// Const-prop fixpoint analysis pinned to the lowered stage.
pub(super) struct ToyConstPropLowered;

impl InterpreterProfile for ToyConstPropLowered {
    type Stage = Stage;
    type Value = ConstProp;
    type Frame = ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>;
    type Completion = ToyCompletion<ConstProp>;
    type Error = ToyError;
}

impl FixpointProfile for ToyConstPropLowered {
    type SummaryKey = ConstPropOwner;
    type Summary = ConstPropSummary<ConstProp, ScfForConstPropSummary<ConstProp>>;
}
