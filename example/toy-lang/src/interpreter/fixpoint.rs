use kirin::prelude::{Pipeline, Symbol};
use kirin_constprop::{ConstPropDriver, ConstPropFunctionFixpoint};
use kirin_function::interpreter::{CallTargetResolution, ResolvedCallTarget};
use kirin_interpreter::{AbstractEnvStore, Location};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

use super::ConstProp;
use super::ToyError;
use super::cross_stage::resolve_cross_stage_call_target;
use super::profile::ToyConstPropFunction;

pub(super) type FunctionFixpoint<'ir> = ConstPropFunctionFixpoint<'ir, ToyConstPropFunction>;

impl<'ir> CallTargetResolution<HighLevel> for FunctionFixpoint<'ir> {
    type Error = ToyError;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error> {
        resolve_cross_stage_call_target::<HighLevel>(self.pipeline(), location, target)
    }
}

impl<'ir> CallTargetResolution<LowLevel> for FunctionFixpoint<'ir> {
    type Error = ToyError;

    fn resolve_call_target(
        &self,
        location: Location,
        target: Symbol,
    ) -> Result<ResolvedCallTarget, Self::Error> {
        resolve_cross_stage_call_target::<LowLevel>(self.pipeline(), location, target)
    }
}

pub fn analyze_source_constprop_fixpoint(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let mut interp: FunctionFixpoint<'_> =
        ConstPropFunctionFixpoint::new(pipeline, AbstractEnvStore::new(), ());
    interp.analyze_function_by_name("source", function_name, args.iter().cloned())
}

pub fn analyze_lowered_constprop_fixpoint(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let mut interp: FunctionFixpoint<'_> =
        ConstPropFunctionFixpoint::new(pipeline, AbstractEnvStore::new(), ());
    interp.analyze_function_by_name("lowered", function_name, args.iter().cloned())
}
