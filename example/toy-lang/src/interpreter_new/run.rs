use kirin::prelude::{Function, LiftFrom, Pipeline};
#[cfg(test)]
use kirin_interpreter_new::{AbstractBlockTransfer, AbstractInterpreter};
use kirin_interpreter_new::{ConcreteInterpreter, InterpreterError, StandardCompletion};

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
    let stage = match pipeline.stage_by_name("source") {
        Some(stage) => stage,
        None => {
            return Err(ToyError::lift_from(InterpreterError::Custom(
                "missing source stage",
            )));
        }
    };
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: ConcreteInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, i64>,
        ToyCompletion<i64>,
        ToyError,
        i64,
    > = ConcreteInterpreter::new(pipeline);
    expect_function_return(
        interp
            .invoke(stage)
            .function(function)
            .args(args.iter().copied())?,
    )
}

pub fn run_lowered_i64(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, ToyError> {
    let stage = match pipeline.stage_by_name("lowered") {
        Some(stage) => stage,
        None => {
            return Err(ToyError::lift_from(InterpreterError::Custom(
                "missing lowered stage",
            )));
        }
    };
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: ConcreteInterpreter<
        '_,
        Stage,
        ToyFrame<LowLevel, i64>,
        ToyCompletion<i64>,
        ToyError,
        i64,
    > = ConcreteInterpreter::new(pipeline);
    expect_function_return(
        interp
            .invoke(stage)
            .function(function)
            .args(args.iter().copied())?,
    )
}

#[cfg(test)]
pub fn analyze_source_constprop(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let stage = match pipeline.stage_by_name("source") {
        Some(stage) => stage,
        None => {
            return Err(ToyError::lift_from(InterpreterError::Custom(
                "missing source stage",
            )));
        }
    };
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: AbstractInterpreter<
        '_,
        Stage,
        ToyFrame<HighLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
        ConstProp,
    > = AbstractInterpreter::new(pipeline);
    expect_function_return(
        interp
            .invoke(stage)
            .function(function)
            .args(args.iter().cloned())?,
    )
}

#[cfg(test)]
pub fn analyze_lowered_constprop(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let stage = match pipeline.stage_by_name("lowered") {
        Some(stage) => stage,
        None => {
            return Err(ToyError::lift_from(InterpreterError::Custom(
                "missing lowered stage",
            )));
        }
    };
    let function = resolve_function(pipeline, function_name)?;
    let mut interp: AbstractInterpreter<
        '_,
        Stage,
        ToyFrame<LowLevel, ConstProp, AbstractBlockTransfer<ConstProp>>,
        ToyCompletion<ConstProp>,
        ToyError,
        ConstProp,
    > = AbstractInterpreter::new(pipeline);
    expect_function_return(
        interp
            .invoke(stage)
            .function(function)
            .args(args.iter().cloned())?,
    )
}

pub(crate) fn resolve_function(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
) -> Result<Function, ToyError> {
    let symbol = match pipeline.lookup_symbol(function_name) {
        Some(symbol) => symbol,
        None => {
            return Err(ToyError::lift_from(InterpreterError::Custom(
                "missing function symbol",
            )));
        }
    };
    pipeline
        .function_by_name(symbol)
        .ok_or(InterpreterError::Custom("missing function"))
        .map_err(ToyError::lift_from)
}

pub(crate) fn expect_function_return<V>(completion: ToyCompletion<V>) -> Result<V, ToyError> {
    match completion {
        ToyCompletion::Standard(StandardCompletion::FunctionReturned(value)) => {
            if value.len() != 1 {
                return Err(ToyError::lift_from(
                    InterpreterError::ProductArityMismatch {
                        expected: 1,
                        actual: value.len(),
                    },
                ));
            }
            Ok(value.into_iter().next().unwrap())
        }
        _ => Err(ToyError::lift_from(InterpreterError::Custom(
            "expected function return",
        ))),
    }
}
