//! The demand result: the demanded SSA values of every reached context.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use kirin_interpreter::{
    BodyScope, DemandContext, InterpreterError, LinkTarget, SparseBackwardInterpreter,
};
use kirin_ir::{SSAValue, StageMeta};

use crate::{Demand, DemandCallCtx};

/// The result of [`analyze_demand`](crate::analyze_demand): the demanded SSA
/// values of every callable context the analysis reached, keyed by the
/// engine's context key `K` (default: [`LinkTarget`] plus [`DemandCallCtx`]).
///
/// The root-scoped accessors ([`demanded`](Self::demanded),
/// [`is_demanded`](Self::is_demanded)) read the analyzed root's context;
/// [`contexts`](Self::contexts) lists every reached context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemandResult<K = (LinkTarget, DemandCallCtx)>
where
    K: Eq + Hash,
{
    root: K,
    contexts: HashMap<K, (BodyScope, HashSet<SSAValue>)>,
}

impl<K: Clone + Eq + Hash> DemandResult<K> {
    /// Copy the demanded sets of every context a converged engine discovered.
    pub fn from_engine<S, E, Lk, P>(
        engine: &SparseBackwardInterpreter<'_, S, Demand, E, Lk, P>,
    ) -> Result<Self, InterpreterError>
    where
        S: StageMeta,
        E: From<InterpreterError>,
        P: DemandContext<Demand, Key = K>,
    {
        let root = engine.root().cloned().ok_or(InterpreterError::Custom(
            "demand analysis has no successful root",
        ))?;
        let mut contexts = HashMap::new();
        for context in engine.contexts() {
            let scope = engine.scope(context).ok_or(InterpreterError::Custom(
                "discovered demand context has no scope",
            ))?;
            let demanded = engine
                .facts(context)
                .filter(|(_, fact)| fact.is_live())
                .map(|(value, _)| value)
                .collect();
            contexts.insert(context.clone(), (scope, demanded));
        }
        if !contexts.contains_key(&root) {
            return Err(InterpreterError::Custom(
                "demand root context was never discovered",
            ));
        }
        Ok(Self { root, contexts })
    }

    /// The analyzed root's context key.
    pub fn root(&self) -> &K {
        &self.root
    }

    /// The resolved `(target stage, callable body)` analyzed by the root.
    pub fn root_scope(&self) -> BodyScope {
        self.contexts[&self.root].0
    }

    /// The root's demanded SSA values (the strong-liveness fact).
    pub fn demanded(&self) -> &HashSet<SSAValue> {
        &self.contexts[&self.root].1
    }

    /// `true` iff `value` is transitively needed by a root in the root's
    /// context.
    pub fn is_demanded(&self, value: impl Into<SSAValue>) -> bool {
        self.demanded().contains(&value.into())
    }

    /// Every context the analysis reached, in no particular order.
    pub fn contexts(&self) -> impl Iterator<Item = &K> {
        self.contexts.keys()
    }
}
