//! Edits to specializations that already exist.

use super::error::UnknownSpecialization;

use crate::node::*;
use crate::{BuilderStageInfo, Dialect};

impl<L: Dialect> BuilderStageInfo<L> {
    /// Retire a specialization: it stays in the stage for backedge tracking
    /// but is no longer matched during dispatch.
    ///
    /// Use this when a specialization is superseded by one with a different
    /// signature (for example after its return type is narrowed), which
    /// [`specialize`](Self::specialize) does not report as a duplicate.
    /// Invalidating an already invalidated specialization does nothing.
    ///
    /// Callers should inspect the specialization's backedges to determine what
    /// needs recompilation.
    pub fn invalidate_specialization(
        &mut self,
        specialization: SpecializedFunction,
    ) -> Result<(), UnknownSpecialization> {
        self.specialization_mut(specialization)?.invalidate();
        Ok(())
    }

    /// Record that `caller` calls `callee`, so that redefining `callee` can
    /// find what to recompile. Recording the same caller twice keeps one
    /// entry.
    pub fn add_specialization_backedge(
        &mut self,
        callee: SpecializedFunction,
        caller: SpecializedFunction,
    ) -> Result<(), UnknownSpecialization> {
        self.specialization_mut(caller)?;
        self.specialization_mut(callee)?.add_backedge(caller);
        Ok(())
    }

    fn specialization_mut(
        &mut self,
        specialization: SpecializedFunction,
    ) -> Result<&mut SpecializedFunctionInfo, UnknownSpecialization> {
        let (staged_function, index) = specialization.id();
        self.staged_functions
            .get_mut(staged_function)
            .and_then(|info| info.specializations.get_mut(index))
            .ok_or(UnknownSpecialization(specialization))
    }
}
