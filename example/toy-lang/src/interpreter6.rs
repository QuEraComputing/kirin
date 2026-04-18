use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::HasStageInfo;
use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_6::abstract_domain::BaseDomain;
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

impl<E> Interpretable<E> for LowLevel
where
    E: BaseDomain,
    E::Value: Clone
        + BranchCondition
        + ProductValue
        + 'static
        + Add<Output = E::Value>
        + Sub<Output = E::Value>
        + Mul<Output = E::Value>
        + Neg<Output = E::Value>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = E::Value>
        + BitOr<Output = E::Value>
        + BitXor<Output = E::Value>
        + Not<Output = E::Value>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <E::Value as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <E::Value as CompareValue>::Bool: Into<E::Value>,
    // Restated from BaseDomain's where clause — Rust does not automatically
    // propagate trait where-clause bounds to generic users of the trait.
    E::Effect: Lift<Core<E::Value, E::Cursor>> + Project<Core<E::Value, E::Cursor>> + Lift<()>,
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

// ---------------------------------------------------------------------------
// Abstract interpretation tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod abstract_tests {
    use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

    use kirin::prelude::*;
    use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
    use kirin_bitwise::{CheckedShl, CheckedShr};
    use kirin_cmp::CompareValue;
    use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
    use kirin_interpreter_6::abstract_interp::AbstractInterp;
    use kirin_interval::Interval;

    use crate::language::LowLevel;
    use crate::stage::Stage;

    // -----------------------------------------------------------------------
    // ToyType — simplest possible abstract type domain (two-element lattice)
    //
    //   Bottom < I64
    //
    // Models "what type does this SSA value have?" rather than "what value?".
    // -----------------------------------------------------------------------

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ToyType {
        Bottom,
        I64,
    }

    impl HasBottom for ToyType {
        fn bottom() -> Self {
            ToyType::Bottom
        }
    }

    impl HasTop for ToyType {
        fn top() -> Self {
            ToyType::I64
        }
    }

    impl Lattice for ToyType {
        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (ToyType::I64, _) | (_, ToyType::I64) => ToyType::I64,
                _ => ToyType::Bottom,
            }
        }

        fn meet(&self, other: &Self) -> Self {
            match (self, other) {
                (ToyType::Bottom, _) | (_, ToyType::Bottom) => ToyType::Bottom,
                _ => ToyType::I64,
            }
        }

        fn is_subseteq(&self, other: &Self) -> bool {
            matches!(
                (self, other),
                (ToyType::Bottom, _) | (ToyType::I64, ToyType::I64)
            )
        }
    }

    impl AbstractValue for ToyType {
        fn widen(&self, next: &Self) -> Self {
            self.join(next)
        }

        fn narrow(&self, next: &Self) -> Self {
            self.meet(next)
        }
    }

    impl BranchCondition for ToyType {
        fn is_truthy(&self) -> Option<bool> {
            None
        }
    }

    impl ProductValue for ToyType {
        fn as_product(&self) -> Option<&Product<Self>> {
            None
        }

        fn from_product(product: Product<Self>) -> Self {
            assert_eq!(product.len(), 1);
            product.into_iter().next().unwrap()
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
        fn cmp_eq(&self, other: &Self) -> Self::Bool {
            self.join(other)
        }
        fn cmp_ne(&self, other: &Self) -> Self::Bool {
            self.join(other)
        }
        fn cmp_lt(&self, other: &Self) -> Self::Bool {
            self.join(other)
        }
        fn cmp_le(&self, other: &Self) -> Self::Bool {
            self.join(other)
        }
        fn cmp_gt(&self, other: &Self) -> Self::Bool {
            self.join(other)
        }
        fn cmp_ge(&self, other: &Self) -> Self::Bool {
            self.join(other)
        }
    }

    macro_rules! toytype_arith {
        ($trait:ident, $method:ident, $output:ident) => {
            impl $trait for ToyType {
                type Output = ToyType;
                fn $method(self, rhs: ToyType) -> ToyType {
                    self.join(&rhs)
                }
            }
        };
    }
    toytype_arith!(Add, add, ToyType);
    toytype_arith!(Sub, sub, ToyType);
    toytype_arith!(Mul, mul, ToyType);
    toytype_arith!(BitAnd, bitand, ToyType);
    toytype_arith!(BitOr, bitor, ToyType);
    toytype_arith!(BitXor, bitxor, ToyType);

    impl Neg for ToyType {
        type Output = ToyType;
        fn neg(self) -> ToyType {
            self
        }
    }

    impl Not for ToyType {
        type Output = ToyType;
        fn not(self) -> ToyType {
            self
        }
    }

    impl CheckedDiv for ToyType {
        fn checked_div(self, rhs: Self) -> Option<Self> {
            Some(self.join(&rhs))
        }
    }

    impl CheckedRem for ToyType {
        fn checked_rem(self, rhs: Self) -> Option<Self> {
            Some(self.join(&rhs))
        }
    }

    impl CheckedShl for ToyType {
        fn checked_shl(self, rhs: Self) -> Option<Self> {
            Some(self.join(&rhs))
        }
    }

    impl CheckedShr for ToyType {
        fn checked_shr(self, rhs: Self) -> Option<Self> {
            Some(self.join(&rhs))
        }
    }

    // -----------------------------------------------------------------------
    // Test programs (lowered stage: unstructured CF)
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
    // Helper: run abstract analysis on a @lowered function
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
        LowLevel: kirin_interpreter_6::env::Interpretable<
                AbstractInterp<'static, Stage, LowLevel, V>,
                DialectEffect = kirin_interpreter_6::core::Core<V, ()>,
            >,
    {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        // Leak to get 'static lifetime — acceptable in test code.
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

        let mut interp: AbstractInterp<'static, Stage, LowLevel, V> =
            AbstractInterp::new(pipeline, stage_id);
        interp.analyze(entry_block, args).expect("analysis failed")
    }

    // -----------------------------------------------------------------------
    // Interval tests
    // -----------------------------------------------------------------------

    #[test]
    fn interval_add_known_range() {
        // add([1,3], [2,4]) should give [3,7]
        let result = analyze_lowered::<Interval>(
            ADD_LOWERED,
            "add",
            vec![Interval::new(1, 3), Interval::new(2, 4)],
        );
        assert_eq!(result, Some(Interval::new(3, 7)));
    }

    #[test]
    fn interval_add_constant() {
        // add([5,5], [10,10]) = [15,15]
        let result = analyze_lowered::<Interval>(
            ADD_LOWERED,
            "add",
            vec![Interval::constant(5), Interval::constant(10)],
        );
        assert_eq!(result, Some(Interval::constant(15)));
    }

    #[test]
    fn interval_branch_joins_both_paths() {
        // sign([−5,5]) — condition is unknown, both branches taken.
        // ^neg returns constant(1) = [1,1], ^pos returns constant(0) = [0,0].
        // Join: [0,1].
        let result =
            analyze_lowered::<Interval>(BRANCH_LOWERED, "sign", vec![Interval::new(-5, 5)]);
        assert_eq!(result, Some(Interval::new(0, 1)));
    }

    #[test]
    fn interval_factorial_converges() {
        // Factorial with unknown non-negative input. The loop unrolls via
        // worklist until fixpoint (widening kicks in). We only check that
        // analysis terminates and returns a non-empty result.
        let result =
            analyze_lowered::<Interval>(FACTORIAL_LOWERED, "factorial", vec![Interval::new(0, 10)]);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(!r.is_empty());
    }

    // -----------------------------------------------------------------------
    // ToyType tests
    // -----------------------------------------------------------------------

    #[test]
    fn toytype_add_propagates_i64() {
        let result =
            analyze_lowered::<ToyType>(ADD_LOWERED, "add", vec![ToyType::I64, ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn toytype_add_bottom_input_propagates() {
        // Bottom input propagates through arithmetic.
        let result =
            analyze_lowered::<ToyType>(ADD_LOWERED, "add", vec![ToyType::Bottom, ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn toytype_branch_both_paths_i64() {
        // Both branches return constant(1) or constant(0), both typed I64.
        let result = analyze_lowered::<ToyType>(BRANCH_LOWERED, "sign", vec![ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }
}
