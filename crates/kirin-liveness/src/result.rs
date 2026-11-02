//! The classic liveness result: dense per-point live sets.

use kirin_interpreter::{
    DenseBackwardInterpreter, FactStore, InterpreterError, LinkTarget, ProgramPoint, Scoped,
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
    root: LinkTarget,
    facts: FactStore<Scoped<LinkTarget, ProgramPoint>, LiveSet>,
}

impl DenseLivenessResult {
    /// Copy the facts recorded by a converged dense engine.
    pub fn from_engine<'ir, S, F, Lk>(
        engine: &DenseBackwardInterpreter<'ir, S, LiveSet, InterpreterError, F, Lk>,
        root: LinkTarget,
    ) -> Self
    where
        S: StageMeta,
    {
        Self {
            root,
            facts: engine.facts(),
        }
    }

    /// The resolved callable analyzed by this result.
    pub fn root(&self) -> LinkTarget {
        self.root
    }

    /// The liveness fact recorded at `point`.
    pub fn point_facts(&self, point: Scoped<LinkTarget, ProgramPoint>) -> Option<&LiveSet> {
        self.facts.get(point)
    }
}
