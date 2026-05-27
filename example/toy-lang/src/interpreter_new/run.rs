use kirin::prelude::Pipeline;
#[cfg(test)]
use kirin_interpreter_new::{AbstractBlockTransfer, AbstractInterpreter};
use kirin_interpreter_new::{ConcreteInterpreter, expect_single_function_return};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

#[cfg(test)]
use super::ConstProp;
use super::{ToyCompletion, ToyError, ToyFrame};

pub fn run_source_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let mut interp: ConcreteInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, i64>,
        ToyCompletion<i64>,
        ToyError,
        i64,
    > = ConcreteInterpreter::new(pipeline);
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
    let mut interp: ConcreteInterpreter<
        '_,
        Stage,
        ToyFrame<LowLevel, i64>,
        ToyCompletion<i64>,
        ToyError,
        i64,
    > = ConcreteInterpreter::new(pipeline);
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
    let mut interp: AbstractInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
        ConstProp,
    > = AbstractInterpreter::new(pipeline);
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
    let mut interp: AbstractInterpreter<
        '_,
        Stage,
        ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
        ConstProp,
    > = AbstractInterpreter::new(pipeline);
    expect_single_function_return(interp.run_function_by_name(
        "lowered",
        function_name,
        args.iter().cloned(),
    )?)
}
