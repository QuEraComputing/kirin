/// Methods for intentionally redefining (overwriting) existing functions.
///
/// These consume the error returned by [`BuilderStageInfo::specialize`] or
/// [`BuilderStageInfo::staged_function`] when a duplicate is detected, invalidate the
/// conflicting entries, and register the new definition.
use super::error::{SpecializeError, StagedFunctionError};

use crate::node::*;
use crate::{BuilderStageInfo, Dialect};

impl<L: Dialect> BuilderStageInfo<L> {
    /// Redefine a specialization by consuming a [`SpecializeError`].
    ///
    /// Invalidates all conflicting specializations identified in the error
    /// and registers the new specialization. Returns the new
    /// [`SpecializedFunction`] ID.
    ///
    /// Callers should inspect the [`SpecializeError::conflicting`] backedges
    /// to determine what needs recompilation.
    pub fn redefine_specialization(&mut self, error: SpecializeError<L>) -> SpecializedFunction {
        let staged_function_info = &mut self.0.staged_functions[error.staged_function];

        // Invalidate all conflicting specializations
        for conflict in &error.conflicting {
            let (_, idx) = conflict.id();
            staged_function_info.specializations[idx].invalidate();
        }

        // Push the new specialization
        let id = SpecializedFunction(
            error.staged_function,
            staged_function_info.specializations.len(),
        );
        let specialized_function = SpecializedFunctionInfo::builder()
            .id(id)
            .signature(error.signature)
            .body(error.body)
            .maybe_backedges(error.backedges)
            .new();
        staged_function_info
            .specializations
            .push(specialized_function);
        id
    }

    /// Redefine a staged function by consuming a [`StagedFunctionError`].
    ///
    /// Invalidates all conflicting staged functions identified in the error
    /// and registers the new staged function. Returns the new
    /// [`StagedFunction`] ID.
    ///
    /// Callers should inspect the backedges of the conflicting staged
    /// functions to determine what needs recompilation.
    pub fn redefine_staged_function(&mut self, error: StagedFunctionError<L>) -> StagedFunction {
        // Invalidate all conflicting staged functions
        for &conflict in &error.conflicting {
            let info = &mut self.0.staged_functions[conflict];
            info.invalidate();
        }

        // Allocate the new staged function
        let id = self.staged_functions.next_id();
        let staged_function = StagedFunctionInfo {
            id,
            name: error.name,
            signature: error.signature,
            specializations: error.specializations,
            backedges: error.backedges,
            invalidated: false,
        };
        self.0.staged_functions.alloc(staged_function);
        id
    }
}
