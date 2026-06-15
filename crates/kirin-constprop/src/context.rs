//! Context-sensitive summary keying for constant propagation.
//!
//! The abstract engine keys function summaries through a
//! [`CallContext`](kirin_interpreter::CallContext) policy. The default
//! ([`DefaultPolicy`](kirin_interpreter::DefaultPolicy)) is context-insensitive
//! — every call site of a function shares one summary — which collapses
//! recursion over distinct constants (e.g. `factorial(5)` and `factorial(4)`
//! join their entry arguments to `Top`).
//!
//! [`ConstPropContext`] keys distinct fully-constant argument tuples to
//! distinct summaries, so `factorial(Const(5))` unfolds `5 → 4 → 3 → 2 → 1`
//! precisely (`Const(120)`), while keeping termination via two bounds:
//!
//! - **Non-constant arguments** map to one shared [`CallCtx::Unknown`] summary.
//! - **A per-function context budget** (`max_contexts`): once exceeded, further
//!   distinct tuples fall back to [`CallCtx::Unknown`].
//!
//! The shared `Unknown` summary is where same-key recursion converges (joined →
//! sound `Top`); the join/widen operator itself is delegated unchanged to
//! [`DefaultPolicy`].

use std::collections::{HashMap, HashSet};

use kirin_interpreter::{AbstractControl, CallContext, DefaultPolicy, InterpreterError};
use kirin_ir::{CompileStage, Product, SpecializedFunction};

use crate::ConstPropValue;

/// The call-context component of a context-sensitive summary key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CallCtx {
    /// Every argument was a known constant — a precise, distinct context.
    Args(Vec<i64>),
    /// Arguments were non-constant, or the per-function budget was exhausted:
    /// one shared, conservative context.
    Unknown,
}

/// Bounded arg-tuple context sensitivity for constant propagation.
#[derive(Clone, Debug)]
pub struct ConstPropContext {
    control: DefaultPolicy,
    max_contexts: usize,
    admitted: HashMap<(CompileStage, SpecializedFunction), HashSet<Vec<i64>>>,
}

impl ConstPropContext {
    /// Construct with an explicit per-function distinct-context budget.
    pub fn with_budget(max_contexts: usize) -> Self {
        Self {
            max_contexts,
            ..Self::default()
        }
    }
}

impl Default for ConstPropContext {
    fn default() -> Self {
        Self {
            control: DefaultPolicy::default(),
            max_contexts: 64,
            admitted: HashMap::new(),
        }
    }
}

impl CallContext<ConstPropValue> for ConstPropContext {
    type Key = (CompileStage, SpecializedFunction, CallCtx);

    fn key(
        &mut self,
        stage: CompileStage,
        function: SpecializedFunction,
        args: &Product<ConstPropValue>,
    ) -> Self::Key {
        let ctx = match all_const(args) {
            Some(consts) => {
                let admitted = self.admitted.entry((stage, function)).or_default();
                if admitted.contains(&consts) {
                    CallCtx::Args(consts)
                } else if admitted.len() < self.max_contexts {
                    admitted.insert(consts.clone());
                    CallCtx::Args(consts)
                } else {
                    // Budget exhausted: collapse to the shared context.
                    CallCtx::Unknown
                }
            }
            None => CallCtx::Unknown,
        };
        (stage, function, ctx)
    }
}

impl AbstractControl<ConstPropValue> for ConstPropContext {
    fn merge(
        &self,
        current: &Product<ConstPropValue>,
        incoming: &Product<ConstPropValue>,
        visits: usize,
    ) -> Result<Product<ConstPropValue>, InterpreterError> {
        self.control.merge(current, incoming, visits)
    }
}

/// `Some(consts)` iff every argument is a known constant.
fn all_const(args: &Product<ConstPropValue>) -> Option<Vec<i64>> {
    args.iter().map(|value| value.as_const().copied()).collect()
}
