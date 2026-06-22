//! Interpreters and analyses for the toy language.
//!
//! All execution machinery lives in `kirin-interpreter`; this module only
//! picks value types, error lifting, and linkers. The cross-language behavior
//! (source-stage functions calling lowered-stage-only functions and back) is
//! a linker choice: [`CrossStageLinker`].

mod error;

#[cfg(test)]
mod tests;

pub use error::ToyError;

use kirin::prelude::Pipeline;
use kirin_constprop::ConstPropValue;
use kirin_interpreter::engine::{
    ConcreteInterpreter, CrossStageLinker, SameStageLinker, expect_single,
};

use crate::stage::Stage;

/// Concrete cross-language interpreter over machine integers.
pub type ToyInterpreter<'ir, Lk = CrossStageLinker> =
    ConcreteInterpreter<'ir, Stage, i64, ToyError, Lk>;

/// Cross-language constant propagation.
pub type ToyConstProp<'ir, Lk = CrossStageLinker> =
    kirin_constprop::ConstProp<'ir, Stage, ToyError, Lk>;

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
