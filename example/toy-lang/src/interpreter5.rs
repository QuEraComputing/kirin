use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_5::concrete::ConcreteDomain;
use kirin_interpreter_5::cursor::{Boxed, Execute};
use kirin_interpreter_5::effect::ControlFlow;
use kirin_interpreter_5::env::{Env, Interpretable};
use kirin_interpreter_5::error::InterpreterError;
use kirin_scf::ForLoopValue;
use kirin_scf::interpreter5::cursor::{ForCursor, IfCursor};

use crate::language::{HighLevel, LowLevel};

// ---------------------------------------------------------------------------
// HighLevel: Interpretable<D>
// ---------------------------------------------------------------------------

impl<D, V> Interpretable<D> for HighLevel
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    D::Error: From<InterpreterError>,
    IfCursor<V>: Execute<D>,
    ForCursor<V>: Execute<D>,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            HighLevel::Lexical(op) => op.interpret(domain),
            HighLevel::Structured(op) => op.interpret(domain),
            HighLevel::Constant(op) => op.interpret(domain),
            HighLevel::Arith(op) => op.interpret(domain),
            HighLevel::Cmp(op) => op.interpret(domain),
            HighLevel::Bitwise(op) => op.interpret(domain),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: Interpretable<D>
// ---------------------------------------------------------------------------

impl<D, V> Interpretable<D> for LowLevel
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone
        + BranchCondition
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    D::Error: From<InterpreterError>,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            LowLevel::Lifted(op) => op.interpret(domain),
            LowLevel::Constant(op) => op.interpret(domain),
            LowLevel::Arith(op) => op.interpret(domain),
            LowLevel::Cmp(op) => op.interpret(domain),
            LowLevel::Bitwise(op) => op.interpret(domain),
            LowLevel::Cf(op) => op.interpret(domain),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — using MultiStageInterp<Stage, i64>
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use kirin::prelude::*;
    use kirin_interpreter_5::concrete::MultiStageInterp;

    use crate::language::HighLevel;
    use crate::stage::Stage;

    fn run_source(src: &str, func_name: &str, args: &[i64]) -> i64 {
        let mut pipeline: Pipeline<Stage> = Pipeline::new();
        ParsePipelineText::parse(&mut pipeline, src).expect("parse failed");

        let stage_id = pipeline
            .stage_by_name("source")
            .expect("stage 'source' not found");
        let stage_container = pipeline.stage(stage_id).expect("stage container");
        let stage_info: &StageInfo<HighLevel> = stage_container
            .try_stage_info()
            .expect("stage type mismatch");
        let staged_fn = pipeline
            .resolve_staged_function(func_name, stage_id)
            .expect("function not found");
        let spec = staged_fn
            .get_info(stage_info)
            .expect("no staged info")
            .unique_live_specialization()
            .expect("no live specialization");
        let spec_info = spec.get_info(stage_info).expect("no spec info");
        let entry_block = spec_info
            .body()
            .definition(stage_info)
            .regions()
            .next()
            .expect("no region")
            .blocks(stage_info)
            .next()
            .expect("no entry block");

        let mut interp = MultiStageInterp::<Stage, i64>::new(&pipeline, stage_id);
        interp
            .enter_function::<HighLevel>(spec, entry_block, args)
            .expect("enter_function failed");
        interp.run().expect("run failed").expect("no return value")
    }

    const ADD: &str = r#"
stage @source fn @main(i64, i64) -> i64;

specialize @source fn @main(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

    const FACTORIAL: &str = r#"
stage @source fn @factorial(i64) -> i64;

specialize @source fn @factorial(i64) -> i64 {
  ^entry(%n: i64) {
    %one = constant 1 -> i64;
    %is_base = le %n, %one -> i64;
    %result = if %is_base then ^then() {
      yield %one;
    } else ^else() {
      %n_minus_1 = sub %n, %one -> i64;
      %rec = call @factorial(%n_minus_1) -> i64;
      %prod = mul %n, %rec -> i64;
      yield %prod;
    } -> i64;
    ret %result;
  }
}
"#;

    const BRANCHING: &str = r#"
stage @source fn @abs(i64) -> i64;

specialize @source fn @abs(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    %result = if %is_neg then ^neg() {
      %neg_x = neg %x -> i64;
      yield %neg_x;
    } else ^pos() {
      yield %x;
    } -> i64;
    ret %result;
  }
}
"#;

    #[test]
    fn test_add() {
        assert_eq!(run_source(ADD, "main", &[3, 5]), 8);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(run_source(ADD, "main", &[-2, 7]), 5);
    }

    #[test]
    fn test_factorial_0() {
        assert_eq!(run_source(FACTORIAL, "factorial", &[0]), 1);
    }

    #[test]
    fn test_factorial_1() {
        assert_eq!(run_source(FACTORIAL, "factorial", &[1]), 1);
    }

    #[test]
    fn test_factorial_5() {
        assert_eq!(run_source(FACTORIAL, "factorial", &[5]), 120);
    }

    #[test]
    fn test_abs_positive() {
        assert_eq!(run_source(BRANCHING, "abs", &[42]), 42);
    }

    #[test]
    fn test_abs_negative() {
        assert_eq!(run_source(BRANCHING, "abs", &[-7]), 7);
    }

    #[test]
    fn test_abs_zero() {
        assert_eq!(run_source(BRANCHING, "abs", &[0]), 0);
    }
}
