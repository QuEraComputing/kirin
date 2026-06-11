use kirin::prelude::Pipeline;
use kirin_interpreter::{ConcreteInterpreter, expect_single_function_return};

use crate::stage::Stage;

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
