//! Bounded output-demand context selection, parallel to constprop's argument policy.

use std::collections::{HashMap, HashSet};

use kirin_interpreter::{DemandContext, LinkTarget};
use kirin_ir::Product;

use crate::Demand;

/// The boundary request distinguishing instances of one callable.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DemandCallCtx {
    /// One demand fact per output, in output order.
    Outputs(Product<Demand>),
    /// Shared fallback after the per-callable specialization budget is exhausted.
    Shared,
}

/// Keep distinct output requests separate up to a per-target budget.
///
/// Admitted requests retain their keys. Further requests join the shared
/// instance; changing demand selects a new key rather than mutating an old one.
#[derive(Clone, Debug)]
pub struct OutputDemandContext {
    max_contexts: usize,
    admitted: HashMap<LinkTarget, HashSet<DemandCallCtx>>,
}

impl Default for OutputDemandContext {
    fn default() -> Self {
        Self::with_budget(64)
    }
}

impl OutputDemandContext {
    pub fn with_budget(max_contexts: usize) -> Self {
        Self {
            max_contexts,
            admitted: HashMap::new(),
        }
    }
}

impl DemandContext<Demand> for OutputDemandContext {
    type Key = (LinkTarget, DemandCallCtx);

    fn key(&mut self, target: &LinkTarget, outputs: &Product<Demand>) -> Self::Key {
        let request = DemandCallCtx::Outputs(outputs.clone());
        let admitted = self.admitted.entry(*target).or_default();
        let context = if admitted.contains(&request) {
            request
        } else if admitted.len() < self.max_contexts {
            admitted.insert(request.clone());
            request
        } else {
            DemandCallCtx::Shared
        };
        (*target, context)
    }
}
