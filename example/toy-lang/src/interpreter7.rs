use std::convert::Infallible;
use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::HasStageInfo;
use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_7::abstract_interp::AbstractInterp;
use kirin_interpreter_7::concrete::ConcreteInterp;
use kirin_interpreter_7::control::{Control, ControlExt};
use kirin_interpreter_7::cursor::{BlockCursor, Execute};
use kirin_interpreter_7::env::{ConcreteEnv, Interpretable};
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::lift::Lift;
use kirin_scf::ForLoopValue;
use kirin_scf::interpreter7::cursor::{ForCursor, IfCursor, SCFCursor};

use crate::stage::Stage;

use crate::language::{HighLevel, LowLevel};

// ---------------------------------------------------------------------------
// HighLevelCursor — typed cursor coproduct for HighLevel
//
// Composes BlockCursor (for function-body blocks) and SCFCursor (for if/for).
// TODO: #[derive(ComposedCursor)] will generate this.
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
// TODO: #[derive(ComposedCursor)] generates this.
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
    E: ConcreteEnv<Value = V, Dialect = HighLevel, Cursor = HighLevelCursor<V>>,
    E::Ext: From<ControlExt<HighLevelCursor<V>>>,
    E::StageContainer: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E, Effect = Control<V, E::Ext>>,
{
    fn execute(&mut self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        match self {
            HighLevelCursor::Block(c) => c.execute(env),
            HighLevelCursor::Scf(c) => c.execute(env),
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
    type Effect = Control<V, ControlExt<HighLevelCursor<V>>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, Stage, HighLevel, V, HighLevelCursor<V>>,
    ) -> Result<Control<V, ControlExt<HighLevelCursor<V>>>, InterpreterError> {
        match self {
            HighLevel::Lexical(op) => op.interpret(env),
            HighLevel::Structured(op) => op.interpret(env),
            HighLevel::Constant(op) => op.interpret(env).map(Control::from),
            HighLevel::Arith(op) => op.interpret(env).map(Control::from),
            HighLevel::Cmp(op) => op.interpret(env).map(Control::from),
            HighLevel::Bitwise(op) => op.interpret(env).map(Control::from),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: Interpretable — concrete and abstract modes
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
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, Stage, LowLevel, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        match self {
            LowLevel::Lifted(op) => op.interpret(env),
            LowLevel::Constant(op) => op.interpret(env).map(Control::from),
            LowLevel::Arith(op) => op.interpret(env).map(Control::from),
            LowLevel::Cmp(op) => op.interpret(env).map(Control::from),
            LowLevel::Bitwise(op) => op.interpret(env).map(Control::from),
            LowLevel::Cf(op) => op.interpret(env),
        }
    }
}

impl<'ir, V> Interpretable<AbstractInterp<'ir, Stage, LowLevel, V>> for LowLevel
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
{
    type Effect = Control<V, Infallible>;

    fn interpret(
        &self,
        env: &mut AbstractInterp<'ir, Stage, LowLevel, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        match self {
            LowLevel::Lifted(op) => op.interpret(env),
            LowLevel::Constant(op) => op.interpret(env).map(Control::from),
            LowLevel::Arith(op) => op.interpret(env).map(Control::from),
            LowLevel::Cmp(op) => op.interpret(env).map(Control::from),
            LowLevel::Bitwise(op) => op.interpret(env).map(Control::from),
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
    use kirin_interpreter::AbstractValue;
    use kirin_interpreter_7::abstract_interp::AbstractInterp;
    use kirin_interpreter_7::concrete::ConcreteInterp;
    use kirin_interval::Interval;

    use crate::interpreter7::HighLevelCursor;
    use crate::language::{HighLevel, LowLevel};
    use crate::stage::Stage;

    use super::*;

    // -----------------------------------------------------------------------
    // Concrete execution helpers
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    fn run_concrete<V>(src: &str, func_name: &str, args: Vec<V>) -> Option<V>
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
        HighLevel: Interpretable<
                ConcreteInterp<'static, Stage, HighLevel, V, HighLevelCursor<V>>,
                Effect = Control<V, ControlExt<HighLevelCursor<V>>>,
            >,
        HighLevelCursor<V>: Execute<ConcreteInterp<'static, Stage, HighLevel, V, HighLevelCursor<V>>>
            + Lift<BlockCursor<V, HighLevel>>,
    {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        let pipeline: &'static Pipeline<Stage> = Box::leak(Box::new(pipeline));

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
        let entry_block = pipeline
            .stage(stage_id)
            .and_then(|s| HasStageInfo::<HighLevel>::try_stage_info(s))
            .and_then(|si| {
                let spec_info = spec.get_info(si)?;
                let body_stmt = *spec_info.body();
                let def = body_stmt.definition(si).clone();
                def.regions().next().and_then(|r| r.blocks(si).next())
            })
            .expect("entry block not found");

        let mut interp: ConcreteInterp<'static, Stage, HighLevel, V, HighLevelCursor<V>> =
            ConcreteInterp::new(pipeline, stage_id);
        interp
            .enter_function::<HighLevel>(spec, entry_block, &args)
            .expect("enter_function failed");
        interp.run().expect("run failed")
    }

    // -----------------------------------------------------------------------
    // Abstract analysis helper
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
        LowLevel: Interpretable<
                AbstractInterp<'static, Stage, LowLevel, V>,
                Effect = Control<V, std::convert::Infallible>,
            >,
    {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        let pipeline: &'static Pipeline<Stage> = Box::leak(Box::new(pipeline));

        let stage_id = pipeline
            .stage_by_name("lowered")
            .expect("stage 'lowered' not found");
        let stage_container = pipeline.stage(stage_id).expect("stage container");
        let stage_info: &StageInfo<LowLevel> = stage_container
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
        let mut interp: AbstractInterp<'static, Stage, LowLevel, V> =
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
    // ToyType: a simple type lattice for abstract interpretation
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
            match (self, other) {
                (_, ToyType::Top) => true,
                (ToyType::Bottom, _) => true,
                (a, b) => a == b,
            }
        }
    }

    impl kirin::prelude::HasBottom for ToyType {
        fn bottom() -> Self {
            ToyType::Bottom
        }
    }

    impl AbstractValue for ToyType {
        fn widen(&self, next: &Self) -> Self {
            // For this finite lattice, widening equals join (terminates immediately).
            self.join(next)
        }
    }

    impl BranchCondition for ToyType {
        fn is_truthy(&self) -> Option<bool> {
            None // conservative: both branches reachable
        }
    }

    impl ProductValue for ToyType {
        fn as_product(&self) -> Option<&kirin::prelude::Product<Self>> {
            None
        }
        fn from_product(_product: kirin::prelude::Product<Self>) -> Self {
            // For abstract type propagation, a product of types collapses to Top
            // (conservative: any combination is possible).
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
        fn checked_div(self, _rhs: Self) -> Option<Self> {
            Some(self)
        }
    }
    impl CheckedRem for ToyType {
        fn checked_rem(self, _rhs: Self) -> Option<Self> {
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
        fn checked_shl(self, _rhs: Self) -> Option<Self> {
            Some(self)
        }
    }
    impl CheckedShr for ToyType {
        fn checked_shr(self, _rhs: Self) -> Option<Self> {
            Some(self)
        }
    }

    // -----------------------------------------------------------------------
    // Concrete tests (HighLevel / SCF / source stage)
    // -----------------------------------------------------------------------

    #[test]
    fn test_add() {
        // Test pure arithmetic via HighLevel concrete interpreter
        // Uses flat CF only (no SCF) for simplicity — run via lowered stage
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, ADD_LOWERED).expect("parse failed");
            p
        };
        let pipeline: &'static Pipeline<Stage> = Box::leak(Box::new(pipeline));
        let stage_id = pipeline.stage_by_name("lowered").unwrap();
        let stage_info: &StageInfo<LowLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function("add", stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let entry_block = {
            let si: &StageInfo<LowLevel> =
                pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
            let spec_info = spec.get_info(si).unwrap();
            let body_stmt = *spec_info.body();
            let def = body_stmt.definition(si).clone();
            def.regions()
                .next()
                .and_then(|r| r.blocks(si).next())
                .unwrap()
        };

        type LLCursor<V> = BlockCursor<V, LowLevel>;
        let mut interp: ConcreteInterp<'static, Stage, LowLevel, i64, LLCursor<i64>> =
            ConcreteInterp::new(pipeline, stage_id);
        interp
            .enter_function::<LowLevel>(spec, entry_block, &[3i64, 5i64])
            .unwrap();
        let result = interp.run().unwrap();
        assert_eq!(result, Some(8i64));
    }

    #[test]
    fn test_add_highlevel() {
        // add(3, 5) = 8 via HighLevel interpreter
        let result = run_concrete_i64_highlevel(ADD_SOURCE, "add", &[3i64, 5i64]);
        assert_eq!(result, Some(8));
    }

    #[test]
    fn test_factorial() {
        // factorial(5) = 120
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
    // Abstract tests (LowLevel / flat CF / lowered stage)
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
    fn interval_add_constant() {
        let result = analyze_lowered::<Interval>(
            ADD_LOWERED,
            "add",
            vec![Interval::constant(5), Interval::constant(10)],
        );
        assert_eq!(result, Some(Interval::constant(15)));
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

    #[test]
    fn toytype_branch_both_paths_i64() {
        let result = analyze_lowered::<ToyType>(BRANCH_LOWERED, "sign", vec![ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }
}
