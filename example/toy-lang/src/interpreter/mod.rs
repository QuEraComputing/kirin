//! Interpreters and analyses for the toy language.
//!
//! All execution machinery lives in `kirin-interpreter`; this module only
//! picks value types, error lifting, and linkers. The cross-language behavior
//! (source-stage functions calling lowered-stage-only functions and back) is
//! a linker choice: [`CrossStageLinker`].

mod error;
mod frame;

#[cfg(test)]
mod tests;

pub use error::ToyError;
pub use frame::{ToyAbstractFrame, ToyFrame};

use kirin::prelude::Pipeline;
use kirin_constprop::{ConstPropContext, ConstPropValue};
use kirin_interpreter::engine::{
    CallContext, ConcreteInterpreter, CrossStageLinker, ForwardAbstractInterpreter,
    SameStageLinker, expect_single,
};

use crate::stage::Stage;

/// Summary key of the constant-propagation analysis policy.
type CpKey = <ConstPropContext as CallContext<ConstPropValue>>::Key;

/// Concrete cross-language interpreter over machine integers. Its frame type
/// embeds the SCF loop frame (the toy language uses `scf.for`).
pub type ToyInterpreter<'ir, Lk = CrossStageLinker> =
    ConcreteInterpreter<'ir, Stage, i64, ToyError, Lk, ToyFrame<i64, ToyError>>;

/// Cross-language constant propagation, with a frame type embedding the SCF
/// loop frame.
pub type ToyConstProp<'ir, Lk = CrossStageLinker> = ForwardAbstractInterpreter<
    'ir,
    Stage,
    ConstPropValue,
    ToyError,
    Lk,
    ConstPropContext,
    ToyAbstractFrame<ConstPropValue, ToyError, CpKey>,
>;

/// Execute `function_name` starting at `stage_name`, following calls across
/// language boundaries.
pub fn run_i64(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let mut interp: ToyInterpreter<'_> =
        ConcreteInterpreter::new(pipeline).with_linker(CrossStageLinker);
    expect_single(interp.call_by_name(stage_name, function_name, args.iter().copied())?)
}

/// Execute within the source stage only.
pub fn run_source_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    run_same_stage_i64(pipeline, "source", function_name, args)
}

/// Execute within the lowered stage only.
pub fn run_lowered_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    run_same_stage_i64(pipeline, "lowered", function_name, args)
}

fn run_same_stage_i64(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let mut interp: ToyInterpreter<'_, SameStageLinker> = ConcreteInterpreter::new(pipeline);
    expect_single(interp.call_by_name(stage_name, function_name, args.iter().copied())?)
}

/// Run constant propagation from `function_name` at `stage_name`, following
/// calls across language boundaries. Returns the function's inferred return
/// value at the fixpoint.
pub fn analyze_constprop(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    function_name: &str,
    args: &[ConstPropValue],
) -> Result<ConstPropValue, ToyError> {
    let mut analysis: ToyConstProp<'_> = ToyConstProp::new(pipeline).with_linker(CrossStageLinker);
    expect_single(analysis.analyze_by_name(stage_name, function_name, args.iter().cloned())?)
}
