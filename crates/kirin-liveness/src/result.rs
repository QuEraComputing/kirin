//! The classic liveness result: dense per-point live sets.

use kirin_interpreter::{
    BodyScope, DenseBackwardInterpreter, FactStore, InterpreterError, ProgramPoint, Scoped,
};
use kirin_ir::StageMeta;

use crate::live::LiveSet;

/// The result of [`analyze_dense`](crate::analyze_dense): classic liveness at
/// every block and statement program point.
///
/// These sets carry the conventional (regalloc-grade) meaning: every use gens,
/// purity-irrelevant.
#[derive(Clone, Debug)]
pub struct DenseLivenessResult {
    root_scope: BodyScope,
    facts: FactStore<Scoped<BodyScope, ProgramPoint>, LiveSet>,
}

impl DenseLivenessResult {
    /// Copy the facts recorded by a converged dense engine.
    pub fn from_engine<'ir, S, F, Lk>(
        engine: &DenseBackwardInterpreter<'ir, S, LiveSet, InterpreterError, F, Lk>,
        root_scope: BodyScope,
    ) -> Self
    where
        S: StageMeta,
    {
        Self {
            root_scope,
            facts: engine.facts(),
        }
    }

    /// The resolved `(target stage, callable body)` analyzed by this result.
    pub fn root_scope(&self) -> BodyScope {
        self.root_scope
    }

    /// The liveness fact recorded at `point`.
    pub fn point_facts(&self, point: Scoped<BodyScope, ProgramPoint>) -> Option<&LiveSet> {
        self.facts.get(point)
    }
}
