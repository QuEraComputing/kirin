//! Owned dense backward analysis results.

use std::marker::PhantomData;

use crate::{
    AnalysisResults, Analyzed, ClassicLiveness, DenseBackwardSemantic, EnvStore, LinkTarget,
    ProgramPoint,
};
use kirin_ir::{Lattice, Product};

/// Completed point facts for one callable, without a program reference.
/// Read through [`Analyzed`] and [`AnalysisResults`].
pub struct DenseBackwardResults<V, Sem: DenseBackwardSemantic = ClassicLiveness> {
    // Preserve rule identity without storing an engine or semantic-key value.
    pub(super) _semantics: PhantomData<fn() -> Sem>,
    pub(super) root: LinkTarget,
    pub(super) env: EnvStore<LinkTarget, ProgramPoint, V>,
}

impl<V: Clone + Lattice, Sem: DenseBackwardSemantic> AnalysisResults
    for Analyzed<DenseBackwardResults<V, Sem>>
{
    type Context = LinkTarget;
    type Anchor = ProgramPoint;
    type Value = V;

    fn root(&self) -> &LinkTarget {
        &self.state.root
    }

    fn contexts(&self) -> impl Iterator<Item = &LinkTarget> {
        std::iter::once(self.root())
    }

    fn callable_of(&self, context: &LinkTarget) -> Option<LinkTarget> {
        (context == self.root()).then_some(*context)
    }

    fn fact(&self, context: &LinkTarget, point: ProgramPoint) -> Option<&V> {
        let env = self.state.env.context_env(context)?;
        self.state.env.environment(env).ok()?.get(point)
    }

    fn facts(&self, context: &LinkTarget) -> impl Iterator<Item = (ProgramPoint, &V)> {
        self.state
            .env
            .context_env(context)
            .and_then(|env| self.state.env.environment(env).ok())
            .into_iter()
            .flat_map(|facts| facts.iter())
    }

    fn response(&self, _context: &LinkTarget) -> Option<Product<V>> {
        None
    }
}
