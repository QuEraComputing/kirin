use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::HasStageInfo;
use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_6::concrete::ConcreteDomain;
use kirin_interpreter_6::core::Core;
use kirin_interpreter_6::cursor::{BlockCursor, Execute};
use kirin_interpreter_6::env::Interpretable;
use kirin_interpreter_6::error::InterpreterError;
use kirin_interpreter_6::lift::{Lift, Project};
use kirin_scf::ForLoopValue;
use kirin_scf::interpreter6::cursor::{ForCursor, IfCursor, SCFCursor};

use crate::language::{HighLevel, LowLevel};

// ---------------------------------------------------------------------------
// HighLevelCursor — typed cursor coproduct for HighLevel
//
// Composes BlockCursor (for function-body blocks) and SCFCursor (for if/for).
// #[derive(ComposedCursor)] generates this. Written manually until the derive exists.
// ---------------------------------------------------------------------------

pub enum HighLevelCursor<V: Clone> {
    Block(BlockCursor<V, HighLevel>),
    Scf(SCFCursor<V, HighLevel>),
}

// Lift: inject each cursor type into the HighLevelCursor coproduct.
impl<V: Clone> Lift<BlockCursor<V, HighLevel>> for HighLevelCursor<V> {
    fn lift(from: BlockCursor<V, HighLevel>) -> Self {
        HighLevelCursor::Block(from)
    }
}
impl<V: Clone> Lift<SCFCursor<V, HighLevel>> for HighLevelCursor<V> {
    fn lift(from: SCFCursor<V, HighLevel>) -> Self {
        HighLevelCursor::Scf(from)
    }
}
// SCFCursor sub-variants inject through SCFCursor.
impl<V: Clone> Lift<IfCursor<V, HighLevel>> for HighLevelCursor<V> {
    fn lift(from: IfCursor<V, HighLevel>) -> Self {
        HighLevelCursor::Scf(SCFCursor::If(from))
    }
}
impl<V: Clone> Lift<ForCursor<V, HighLevel>> for HighLevelCursor<V> {
    fn lift(from: ForCursor<V, HighLevel>) -> Self {
        HighLevelCursor::Scf(SCFCursor::For(from))
    }
}

// Execute<E> for HighLevelCursor: dispatch to inner cursor.
// #[derive(ComposedCursor)] generates this. Written manually until the derive exists.
impl<E, V> Execute<E> for HighLevelCursor<V>
where
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
    E: ConcreteDomain<Value = V, Language = HighLevel, Cursor = HighLevelCursor<V>>,
    E::Effect: Lift<Core<V, HighLevelCursor<V>>> + Project<Core<V, HighLevelCursor<V>>> + Lift<()>,
    E::StageContainer: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E, DialectEffect = E::Effect>,
{
    fn execute(&mut self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            HighLevelCursor::Block(c) => c.execute(env),
            HighLevelCursor::Scf(c) => c.execute(env),
        }
    }
}

// ---------------------------------------------------------------------------
// HighLevel: Interpretable<E>
// ---------------------------------------------------------------------------

impl<E, V> Interpretable<E> for HighLevel
where
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
    E: ConcreteDomain<Value = V, Language = HighLevel, Cursor = HighLevelCursor<V>>,
    HighLevelCursor<V>: Lift<BlockCursor<V, HighLevel>>
        + Lift<SCFCursor<V, HighLevel>>
        + Lift<IfCursor<V, HighLevel>>
        + Lift<ForCursor<V, HighLevel>>,
    E::Effect: Lift<Core<V, HighLevelCursor<V>>> + Project<Core<V, HighLevelCursor<V>>> + Lift<()>,
    E::StageContainer: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            HighLevel::Lexical(op) => op.interpret(env),
            HighLevel::Structured(op) => op.interpret(env),
            HighLevel::Constant(op) => op.interpret(env),
            HighLevel::Arith(op) => op.interpret(env),
            HighLevel::Cmp(op) => op.interpret(env),
            HighLevel::Bitwise(op) => op.interpret(env),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: Interpretable<E>
// ---------------------------------------------------------------------------

impl<E, V> Interpretable<E> for LowLevel
where
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
    E: ConcreteDomain<Value = V>,
    E::Effect: Lift<Core<V, E::Cursor>> + Project<Core<V, E::Cursor>> + Lift<()>,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            LowLevel::Lifted(op) => op.interpret(env),
            LowLevel::Constant(op) => op.interpret(env),
            LowLevel::Arith(op) => op.interpret(env),
            LowLevel::Cmp(op) => op.interpret(env),
            LowLevel::Bitwise(op) => op.interpret(env),
            LowLevel::Cf(op) => op.interpret(env),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use kirin::prelude::*;
    use kirin_interpreter_6::concrete::ConcreteInterp;
    use kirin_interpreter_6::core::Core;

    use crate::interpreter6::HighLevelCursor;
    use crate::language::HighLevel;
    use crate::stage::Stage;

    type MyCursor = HighLevelCursor<i64>;
    type MyEff = Core<i64, MyCursor>;
    type MyInterp<'ir> = ConcreteInterp<'ir, Stage, HighLevel, i64, MyCursor, MyEff>;

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

        let mut interp = MyInterp::new(&pipeline, stage_id);
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
