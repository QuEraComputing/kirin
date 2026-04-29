use kirin::prelude::{Function, Pipeline};
#[cfg(test)]
use kirin_interpreter_new::AbstractInterpreter;
use kirin_interpreter_new::{
    ConcreteInterpreter, FunctionFrame, InterpreterError, StandardCompletion,
};

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
    let stage = pipeline
        .stage_by_name("source")
        .ok_or(InterpreterError::Custom("missing source stage"))?;
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: ConcreteInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, i64>,
        ToyCompletion<i64>,
        ToyError,
        i64,
    > = ConcreteInterpreter::new(pipeline);
    interp.push_frame(FunctionFrame::<HighLevel, i64>::new(stage, function, args.to_vec()).into());
    expect_function_return(interp.run()?)
}

pub fn run_lowered_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let stage = pipeline
        .stage_by_name("lowered")
        .ok_or(InterpreterError::Custom("missing lowered stage"))?;
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: ConcreteInterpreter<
        '_,
        Stage,
        ToyFrame<LowLevel, i64>,
        ToyCompletion<i64>,
        ToyError,
        i64,
    > = ConcreteInterpreter::new(pipeline);
    interp.push_frame(FunctionFrame::<LowLevel, i64>::new(stage, function, args.to_vec()).into());
    expect_function_return(interp.run()?)
}

#[cfg(test)]
pub fn analyze_source_constprop(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let stage = pipeline
        .stage_by_name("source")
        .ok_or(InterpreterError::Custom("missing source stage"))?;
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: AbstractInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, ConstProp>,
        ToyCompletion<ConstProp>,
        ToyError,
        ConstProp,
    > = AbstractInterpreter::new(pipeline);
    interp.push_frame(
        FunctionFrame::<HighLevel, ConstProp>::new(stage, function, args.to_vec()).into(),
    );
    expect_function_return(interp.run()?)
}

#[cfg(test)]
pub fn analyze_lowered_constprop(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let stage = pipeline
        .stage_by_name("lowered")
        .ok_or(InterpreterError::Custom("missing lowered stage"))?;
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: AbstractInterpreter<
        '_,
        Stage,
        ToyFrame<LowLevel, ConstProp>,
        ToyCompletion<ConstProp>,
        ToyError,
        ConstProp,
    > = AbstractInterpreter::new(pipeline);
    interp.push_frame(
        FunctionFrame::<LowLevel, ConstProp>::new(stage, function, args.to_vec()).into(),
    );
    expect_function_return(interp.run()?)
}

pub(crate) fn resolve_function(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
) -> Result<Function, ToyError> {
    let symbol = pipeline
        .lookup_symbol(function_name)
        .ok_or(InterpreterError::Custom("missing function symbol"))?;
    pipeline
        .function_by_name(symbol)
        .ok_or(InterpreterError::Custom("missing function"))
        .map_err(ToyError::from)
}

pub(crate) fn expect_function_return<V>(completion: ToyCompletion<V>) -> Result<V, ToyError> {
    match completion {
        ToyCompletion::Standard(StandardCompletion::FunctionReturned(value)) => Ok(value),
        _ => Err(InterpreterError::Custom("expected function return").into()),
    }
}
