//! Test-only single-run abstract interpretation helpers.
//!
//! These run the abstract interpreter once over a function (no fixpoint
//! iteration). They are used to verify per-statement transfer functions in
//! isolation, without the owner-summary plumbing exercised by the fixpoint
//! driver.

use kirin::prelude::Pipeline;
use kirin_constprop::{ConstPropDriver, ConstPropFunctionFixpoint};
use kirin_interpreter::{
    AbstractEnvStore, AbstractInterpreter, FunctionInvocation, expect_single_function_return,
};

use crate::interpreter::{ConstProp, ToyError};
use crate::stage::Stage;

use super::profile::{ToyLoweredConstProp, ToySourceConstProp};

pub(super) fn analyze_source_constprop(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let mut interp = AbstractInterpreter::<ToySourceConstProp>::new(pipeline);
    expect_single_function_return(interp.run_function_by_name(
        "source",
        function_name,
        args.iter().cloned(),
    )?)
}

pub(super) fn analyze_lowered_constprop(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let mut interp = AbstractInterpreter::<ToyLoweredConstProp>::new(pipeline);
    expect_single_function_return(interp.run_function_by_name(
        "lowered",
        function_name,
        args.iter().cloned(),
    )?)
}

/// Run the source-stage const-prop fixpoint driver on a specific
/// [`FunctionInvocation`] variant.
///
/// Used by `constprop_fixpoint_source_entry_variants` to verify that all three
/// `FunctionInvocation` constructors (function / staged / specialized) reach
/// the same summary value.
pub(super) fn analyze_source_constprop_invocation(
    pipeline: &Pipeline<Stage>,
    invocation: FunctionInvocation<ConstProp>,
) -> Result<ConstProp, ToyError> {
    let mut interp: crate::interpreter::fixpoint::FunctionFixpoint<'_> =
        ConstPropFunctionFixpoint::new(pipeline, AbstractEnvStore::new(), ());
    interp.analyze_function(
        invocation.stage(),
        invocation.target(),
        invocation.args().iter().cloned(),
    )
}
