use kirin_ir::{Dialect, Product, StageInfo, StageMeta, Statement};

use crate::{FunctionBody, Interp};

/// Statement semantics. The single trait dialect authors implement.
///
/// The engine type `I` is the object a rule receives directly; the `Kind`
/// parameter is a compile-time semantics marker (e.g.
/// [`SparseForward`](crate::SparseForward)) that selects *which* semantics this
/// impl describes. The same dialect type can carry one impl per `Kind` — e.g. a
/// forward-value rule and a future backward-liveness rule — without coherence
/// conflicts.
pub trait Interpretable<I: Interp, Kind>: Dialect {
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error>;
}

/// Function-entry semantics for callable statements.
///
/// Implemented by statements that define function bodies (e.g.
/// `kirin_function::Function`); describes the [`FunctionBody`] an engine enters
/// when the function is invoked. Derived on language enums with
/// `#[derive(FunctionEntry)]` where `#[callable]` marks the variants that wrap
/// callable statements.
pub trait FunctionEntry<I: Interp>: Dialect {
    fn function_entry(
        &self,
        args: Product<I::Value>,
        interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error>;
}

/// Monomorphic statement dispatch over a stage enum.
///
/// Mirrors `ParseDispatch` from the parser: multi-stage pipelines add
/// `#[derive(InterpDispatch)]` to their stage enum; single-language pipelines
/// (`Pipeline<StageInfo<L>>`) get the blanket impl below. Engines route every
/// statement execution and function entry through this trait; compiler
/// authors derive it and never call it. Keyed on the engine `I` and the
/// semantics `Kind`.
pub trait InterpDispatch<I: Interp, Kind>: StageMeta {
    fn dispatch_statement(
        &self,
        statement: Statement,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error>;

    fn dispatch_function_entry(
        &self,
        body: Statement,
        args: Product<I::Value>,
        interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error>;
}

impl<I, Kind, L> InterpDispatch<I, Kind> for StageInfo<L>
where
    I: Interp,
    L: Dialect + Interpretable<I, Kind> + FunctionEntry<I>,
{
    fn dispatch_statement(
        &self,
        statement: Statement,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error> {
        let definition = statement.definition(self).clone();
        definition.interpret(interp)
    }

    fn dispatch_function_entry(
        &self,
        body: Statement,
        args: Product<I::Value>,
        interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        let definition = body.definition(self).clone();
        definition.function_entry(args, interp)
    }
}

/// Compile-time regression proof: one dialect carries one `Interpretable`
/// rule per analysis `Kind` — forward evaluation, sparse backward demand, and
/// dense backward liveness — against the *shipped* engine traits, with no
/// coherence conflicts.
#[cfg(test)]
mod tests {
    use std::fmt;

    use kirin_ir::{Dialect, HasBottom};

    use crate::{
        DenseBackward, DenseBackwardEffect, DenseBackwardInterp, Interpretable, SparseBackward,
        SparseBackwardInterp, SparseForward, SparseForwardEffect, SparseForwardInterp,
    };

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct MockType;

    impl fmt::Display for MockType {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("mock")
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Dialect)]
    #[kirin(type = MockType, crate = kirin_ir)]
    enum MockDialect {
        Op,
    }

    // A forward-value rule, written against the forward engine surface.
    impl<I> Interpretable<I, SparseForward> for MockDialect
    where
        I: SparseForwardInterp,
    {
        fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
            Ok(SparseForwardEffect::Next)
        }
    }

    // A backward-demand rule for the *same* dialect, against the shipped
    // sparse backward surface — distinguished only by the `Kind` marker.
    impl<I> Interpretable<I, SparseBackward> for MockDialect
    where
        I: SparseBackwardInterp,
        I::Value: HasBottom + PartialEq,
    {
        fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
            Ok(interp.effect())
        }
    }

    // A dense classic-liveness rule for the *same* dialect, against the
    // shipped dense backward surface.
    impl<I> Interpretable<I, DenseBackward> for MockDialect
    where
        I: DenseBackwardInterp,
    {
        fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
            Ok(DenseBackwardEffect::Next)
        }
    }

    #[test]
    fn one_dialect_carries_a_rule_per_kind() {
        // If this module compiles, the three `Interpretable` impls above pass
        // coherence together: the `Kind` marker is what lets one dialect type
        // carry a rule per analysis. The generic assertions below prove each
        // rule resolves against the real engine trait it is written for (any
        // engine satisfying that trait can dispatch the rule).
        #[allow(dead_code)]
        fn assert_forward_rule<I: SparseForwardInterp>()
        where
            MockDialect: Interpretable<I, SparseForward>,
        {
        }
        #[allow(dead_code)]
        fn assert_demand_rule<I: SparseBackwardInterp>()
        where
            I::Value: HasBottom + PartialEq,
            MockDialect: Interpretable<I, SparseBackward>,
        {
        }
        #[allow(dead_code)]
        fn assert_dense_rule<I: DenseBackwardInterp>()
        where
            MockDialect: Interpretable<I, DenseBackward>,
        {
        }

        let _ = MockType;
        let _ = MockDialect::Op;
    }
}
