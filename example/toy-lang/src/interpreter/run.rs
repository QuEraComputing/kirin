use kirin::prelude::Pipeline;
#[cfg(test)]
use kirin_interpreter::AbstractInterpreter;
use kirin_interpreter::{ConcreteInterpreter, expect_single_function_return};

use crate::stage::Stage;

#[cfg(test)]
use super::ConstProp;
#[cfg(test)]
use super::profile::{ToyLoweredConstProp, ToySourceConstProp};
use super::{ToyError, ToyLoweredConcrete, ToySourceConcrete};

pub fn run_source_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let mut interp = ConcreteInterpreter::<ToySourceConcrete>::new(pipeline);
    expect_single_function_return(interp.run_function_by_name(
        "source",
        function_name,
        args.iter().copied(),
    )?)
}

pub fn run_lowered_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let mut interp = ConcreteInterpreter::<ToyLoweredConcrete>::new(pipeline);
    expect_single_function_return(interp.run_function_by_name(
        "lowered",
        function_name,
        args.iter().copied(),
    )?)
}

#[cfg(test)]
pub fn analyze_source_constprop(
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

#[cfg(test)]
pub fn analyze_lowered_constprop(
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
