use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::HasStageInfo;
use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_function::interpreter9::interpret::eval_call_for_dialect;
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_9::abstract_interp::AbstractInterp;
use kirin_interpreter_9::algebra::Lift;
use kirin_interpreter_9::concrete::ConcreteInterp;
use kirin_interpreter_9::control::{Control, CursorExt};
use kirin_interpreter_9::cursor::{AbstractBlockCursor, BlockCursor};
use kirin_interpreter_9::env::{ConcreteMode, Env};
use kirin_interpreter_9::error::InterpreterError;
use kirin_interpreter_9::execute::Execute;
use kirin_interpreter_9::interpretable::Interpretable;
use kirin_scf::ForLoopValue;
use kirin_scf::StructuredControlFlow;
use kirin_scf::interpreter9::cursor::SCFCursor;
use kirin_scf::interpreter9::interpret::{eval_for_concrete, eval_if_concrete};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

// ---------------------------------------------------------------------------
// HighLevelCursor — typed cursor coproduct for HighLevel (concrete mode)
// ---------------------------------------------------------------------------

pub enum HighLevelCursor<V: Clone> {
    Block(BlockCursor<V, HighLevel>),
    Scf(SCFCursor<V, HighLevel>),
}

impl<V: Clone> Lift<HighLevelCursor<V>> for BlockCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Block(self)
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>> for SCFCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(self)
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>>
    for kirin_scf::interpreter9::cursor::IfCursor<V, HighLevel>
{
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(SCFCursor::If(self))
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>>
    for kirin_scf::interpreter9::cursor::ForCursor<V, HighLevel>
{
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(SCFCursor::For(self))
    }
}

// Execute<E> for HighLevelCursor: dispatch to inner cursor.
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
    E: Env<Mode = ConcreteMode<HighLevelCursor<V>>, Value = V, Ext = CursorExt<HighLevelCursor<V>>>,
    E::Stages: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<HighLevelCursor<V>>>, E::Error> {
        match self {
            HighLevelCursor::Block(c) => c.execute(env, inbox),
            HighLevelCursor::Scf(c) => c.execute(env, inbox),
        }
    }
}

// ---------------------------------------------------------------------------
// HighLevel: Interpretable<ConcreteInterp<...>>
// ---------------------------------------------------------------------------

impl<'ir, V> Interpretable<ConcreteInterp<'ir, Stage, HighLevel, V, HighLevelCursor<V>>>
    for HighLevel
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
{
    fn eval(
        &self,
        env: &mut ConcreteInterp<'ir, Stage, HighLevel, V, HighLevelCursor<V>>,
    ) -> Result<Control<V, CursorExt<HighLevelCursor<V>>>, InterpreterError> {
        match self {
            HighLevel::Lexical(op) => match op {
                kirin_function::Lexical::FunctionBody(op) => op.eval(env),
                kirin_function::Lexical::Lambda(op) => op.eval(env),
                kirin_function::Lexical::Call(op) => {
                    eval_call_for_dialect::<_, HighLevel, _>(op, env)
                }
                kirin_function::Lexical::Return(op) => op.eval(env),
            },
            HighLevel::Structured(op) => match op {
                StructuredControlFlow::If(op) => {
                    eval_if_concrete::<_, HighLevelCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::For(op) => {
                    eval_for_concrete::<_, HighLevelCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::Yield(op) => op.eval(env),
            },
            HighLevel::Constant(op) => op.eval(env),
            HighLevel::Arith(op) => op.eval(env),
            HighLevel::Cmp(op) => op.eval(env),
            HighLevel::Bitwise(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevelAbstractCursor — for abstract mode (flat CF, no SCF)
//
// We use AbstractBlockCursor<V, LowLevel> directly as C; the identity Lift
// impl from algebra.rs (impl<T> Lift<T> for T) makes this work.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub type LowLevelAbstractCursor<V> = AbstractBlockCursor<V, LowLevel>;

// ---------------------------------------------------------------------------
// LowLevel: Interpretable — concrete mode
// ---------------------------------------------------------------------------

impl<'ir, V, C> Interpretable<ConcreteInterp<'ir, Stage, LowLevel, V, C>> for LowLevel
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
    C: 'static,
{
    fn eval(
        &self,
        env: &mut ConcreteInterp<'ir, Stage, LowLevel, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        match self {
            LowLevel::Lifted(op) => match op {
                kirin_function::Lifted::FunctionBody(op) => op.eval(env),
                kirin_function::Lifted::Bind(op) => op.eval(env),
                kirin_function::Lifted::Call(op) => {
                    eval_call_for_dialect::<_, LowLevel, _>(op, env)
                }
                kirin_function::Lifted::Return(op) => op.eval(env),
            },
            LowLevel::Constant(op) => op.eval(env),
            LowLevel::Arith(op) => op.eval(env),
            LowLevel::Cmp(op) => op.eval(env),
            LowLevel::Bitwise(op) => op.eval(env),
            LowLevel::Cf(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: Interpretable — abstract mode
// ---------------------------------------------------------------------------

impl<'ir, V, C> Interpretable<AbstractInterp<'ir, Stage, LowLevel, V, C>> for LowLevel
where
    V: Clone
        + AbstractValue
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
    C: 'static,
{
    fn eval(
        &self,
        env: &mut AbstractInterp<'ir, Stage, LowLevel, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        match self {
            LowLevel::Lifted(op) => match op {
                kirin_function::Lifted::FunctionBody(op) => op.eval(env),
                kirin_function::Lifted::Bind(op) => op.eval(env),
                kirin_function::Lifted::Call(op) => {
                    eval_call_for_dialect::<_, LowLevel, _>(op, env)
                }
                kirin_function::Lifted::Return(op) => op.eval(env),
            },
            LowLevel::Constant(op) => op.eval(env),
            LowLevel::Arith(op) => op.eval(env),
            LowLevel::Cmp(op) => op.eval(env),
            LowLevel::Bitwise(op) => op.eval(env),
            LowLevel::Cf(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use kirin::prelude::*;
    use kirin_interpreter::AbstractValue;
    use kirin_interpreter_9::abstract_interp::AbstractInterp;
    use kirin_interpreter_9::concrete::ConcreteInterp;
    use kirin_interval::Interval;

    use crate::interpreter9::{HighLevelCursor, LowLevelAbstractCursor};
    use crate::language::{HighLevel, LowLevel};
    use crate::stage::Stage;

    use super::*;

    // -----------------------------------------------------------------------
    // Concrete execution helper (HighLevel / source stage)
    // -----------------------------------------------------------------------

    fn run_concrete_i64_highlevel(src: &str, func_name: &str, args: &[i64]) -> Option<i64> {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        let pipeline: &'static Pipeline<Stage> = Box::leak(Box::new(pipeline));
        let stage_id = pipeline.stage_by_name("source").unwrap();
        let stage_info: &StageInfo<HighLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function(func_name, stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let entry_block = {
            let si: &StageInfo<HighLevel> =
                pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
            let spec_info = spec.get_info(si).unwrap();
            let body_stmt = *spec_info.body();
            let def = body_stmt.definition(si).clone();
            def.regions()
                .next()
                .and_then(|r| r.blocks(si).next())
                .unwrap()
        };
        let mut interp: ConcreteInterp<'static, Stage, HighLevel, i64, HighLevelCursor<i64>> =
            ConcreteInterp::new(pipeline, stage_id);
        interp
            .enter_function::<HighLevel>(spec, entry_block, args)
            .unwrap();
        interp.run().unwrap()
    }

    // -----------------------------------------------------------------------
    // Abstract analysis helper (LowLevel / lowered stage)
    // -----------------------------------------------------------------------

    fn analyze_lowered<V>(src: &str, func_name: &str, args: Vec<V>) -> Option<V>
    where
        V: Clone
            + AbstractValue
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
        LowLevel:
            Interpretable<AbstractInterp<'static, Stage, LowLevel, V, LowLevelAbstractCursor<V>>>,
        AbstractBlockCursor<V, LowLevel>:
            Execute<AbstractInterp<'static, Stage, LowLevel, V, LowLevelAbstractCursor<V>>>,
    {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        let pipeline: &'static Pipeline<Stage> = Box::leak(Box::new(pipeline));
        let stage_id = pipeline.stage_by_name("lowered").unwrap();
        let stage_info: &StageInfo<LowLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function(func_name, stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let mut interp: AbstractInterp<'static, Stage, LowLevel, V, LowLevelAbstractCursor<V>> =
            AbstractInterp::new(pipeline, stage_id);
        interp.analyze(spec, args).expect("analysis failed")
    }

    // -----------------------------------------------------------------------
    // Source programs (HighLevel / SCF)
    // -----------------------------------------------------------------------

    const ADD_SOURCE: &str = r#"
stage @source fn @add(i64, i64) -> i64;

specialize @source fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

    const FACTORIAL_SOURCE: &str = r#"
stage @source fn @factorial(i64) -> i64;

specialize @source fn @factorial(i64) -> i64 {
  ^entry(%n: i64) {
    %one = constant 1 -> i64;
    %is_base = le %n, %one -> i64;
    %result = if %is_base then ^base() {
      yield %one;
    } else ^recurse() {
      %n_minus_1 = sub %n, %one -> i64;
      %rec = call @factorial(%n_minus_1) -> i64;
      %prod = mul %n, %rec -> i64;
      yield %prod;
    } -> i64;
    ret %result;
  }
}
"#;

    const ABS_SOURCE: &str = r#"
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

    // -----------------------------------------------------------------------
    // Lowered programs (flat CF)
    // -----------------------------------------------------------------------

    const ADD_LOWERED: &str = r#"
stage @lowered fn @add(i64, i64) -> i64;

specialize @lowered fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

    const BRANCH_LOWERED: &str = r#"
stage @lowered fn @sign(i64) -> i64;

specialize @lowered fn @sign(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    cond_br %is_neg then=^neg() else=^pos();
  }
  ^neg() {
    %one = constant 1 -> i64;
    ret %one;
  }
  ^pos() {
    %zero2 = constant 0 -> i64;
    ret %zero2;
  }
}
"#;

    const FACTORIAL_LOWERED: &str = r#"
stage @lowered fn @factorial(i64) -> i64;

specialize @lowered fn @factorial(i64) -> i64 {
  ^entry(%n: i64) {
    %one = constant 1 -> i64;
    %is_base = le %n, %one -> i64;
    cond_br %is_base then=^base() else=^recurse();
  }
  ^base() {
    %one2 = constant 1 -> i64;
    ret %one2;
  }
  ^recurse() {
    %one3 = constant 1 -> i64;
    %n_minus_1 = sub %n, %one3 -> i64;
    %rec = call @factorial(%n_minus_1) -> i64;
    %prod = mul %n, %rec -> i64;
    ret %prod;
  }
}
"#;

    // -----------------------------------------------------------------------
    // ToyType: simple type lattice for abstract interpretation
    // -----------------------------------------------------------------------

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ToyType {
        Bottom,
        I64,
        Bool,
        Top,
    }

    impl kirin::prelude::Lattice for ToyType {
        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (ToyType::Bottom, x) | (x, ToyType::Bottom) => x.clone(),
                (a, b) if a == b => a.clone(),
                _ => ToyType::Top,
            }
        }
        fn meet(&self, other: &Self) -> Self {
            match (self, other) {
                (ToyType::Top, x) | (x, ToyType::Top) => x.clone(),
                (a, b) if a == b => a.clone(),
                _ => ToyType::Bottom,
            }
        }
        fn is_subseteq(&self, other: &Self) -> bool {
            matches!((self, other), (_, ToyType::Top) | (ToyType::Bottom, _)) || self == other
        }
    }

    impl kirin::prelude::HasBottom for ToyType {
        fn bottom() -> Self {
            ToyType::Bottom
        }
    }

    impl AbstractValue for ToyType {
        fn widen(&self, next: &Self) -> Self {
            self.join(next)
        }
    }

    impl BranchCondition for ToyType {
        fn is_truthy(&self) -> Option<bool> {
            None
        }
    }

    impl ProductValue for ToyType {
        fn as_product(&self) -> Option<&kirin::prelude::Product<Self>> {
            None
        }
        fn from_product(_product: kirin::prelude::Product<Self>) -> Self {
            ToyType::Top
        }
    }

    impl TryFrom<ArithValue> for ToyType {
        type Error = std::convert::Infallible;
        fn try_from(_: ArithValue) -> Result<Self, Self::Error> {
            Ok(ToyType::I64)
        }
    }

    impl CompareValue for ToyType {
        type Bool = ToyType;
        fn cmp_eq(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_ne(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_lt(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_le(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_gt(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_ge(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
    }

    impl std::ops::Add for ToyType {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::Sub for ToyType {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::Mul for ToyType {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::Neg for ToyType {
        type Output = Self;
        fn neg(self) -> Self {
            self
        }
    }
    impl CheckedDiv for ToyType {
        fn checked_div(self, _: Self) -> Option<Self> {
            Some(self)
        }
    }
    impl CheckedRem for ToyType {
        fn checked_rem(self, _: Self) -> Option<Self> {
            Some(self)
        }
    }
    impl std::ops::BitAnd for ToyType {
        type Output = Self;
        fn bitand(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::BitOr for ToyType {
        type Output = Self;
        fn bitor(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::BitXor for ToyType {
        type Output = Self;
        fn bitxor(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::Not for ToyType {
        type Output = Self;
        fn not(self) -> Self {
            self
        }
    }
    impl CheckedShl for ToyType {
        fn checked_shl(self, _: Self) -> Option<Self> {
            Some(self)
        }
    }
    impl CheckedShr for ToyType {
        fn checked_shr(self, _: Self) -> Option<Self> {
            Some(self)
        }
    }

    // -----------------------------------------------------------------------
    // Concrete tests (HighLevel / source stage)
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_highlevel() {
        let result = run_concrete_i64_highlevel(ADD_SOURCE, "add", &[3i64, 5i64]);
        assert_eq!(result, Some(8));
    }

    #[test]
    fn test_factorial() {
        let result = run_concrete_i64_highlevel(FACTORIAL_SOURCE, "factorial", &[5i64]);
        assert_eq!(result, Some(120));
    }

    #[test]
    fn test_abs_positive() {
        let result = run_concrete_i64_highlevel(ABS_SOURCE, "abs", &[42i64]);
        assert_eq!(result, Some(42));
    }

    #[test]
    fn test_abs_negative() {
        let result = run_concrete_i64_highlevel(ABS_SOURCE, "abs", &[-7i64]);
        assert_eq!(result, Some(7));
    }

    // -----------------------------------------------------------------------
    // Abstract tests (LowLevel / lowered stage)
    // -----------------------------------------------------------------------

    #[test]
    fn interval_add_known_range() {
        let result = analyze_lowered::<Interval>(
            ADD_LOWERED,
            "add",
            vec![Interval::new(1, 3), Interval::new(2, 4)],
        );
        assert_eq!(result, Some(Interval::new(3, 7)));
    }

    #[test]
    fn interval_branch_joins_both_paths() {
        let result =
            analyze_lowered::<Interval>(BRANCH_LOWERED, "sign", vec![Interval::new(-5, 5)]);
        assert_eq!(result, Some(Interval::new(0, 1)));
    }

    #[test]
    fn interval_factorial_converges() {
        let result =
            analyze_lowered::<Interval>(FACTORIAL_LOWERED, "factorial", vec![Interval::new(0, 10)]);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(!r.is_empty());
    }

    #[test]
    fn toytype_add_propagates_i64() {
        let result =
            analyze_lowered::<ToyType>(ADD_LOWERED, "add", vec![ToyType::I64, ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }
}
