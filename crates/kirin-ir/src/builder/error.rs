use crate::language::Dialect;
use crate::node::function::{SpecializedFunction, SpecializedFunctionInfo, StagedFunction};
use crate::node::stmt::Statement;
use crate::node::symbol::Symbol;
use crate::signature::Signature;

/// Error returned when [`Context::specialize`] detects an existing non-invalidated
/// specialization with the same signature.
///
/// The caller can either propagate this error or consume it via
/// [`Context::redefine_specialization`] to intentionally invalidate the old
/// specialization and register the new one.
#[derive(Debug)]
pub struct SpecializeError<L: Dialect> {
    /// The staged function being specialized.
    pub staged_function: StagedFunction,
    /// The signature that conflicted.
    pub signature: Signature<L::Type>,
    /// Existing non-invalidated specializations with matching signatures.
    pub conflicting: Vec<SpecializedFunction>,
    /// Preserved body statement for the new specialization.
    pub body: Statement,
    /// Preserved backedges for the new specialization.
    pub backedges: Option<Vec<SpecializedFunction>>,
}

impl<L: Dialect> std::fmt::Display for SpecializeError<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "duplicate specialization: {} existing specialization(s) with the same signature",
            self.conflicting.len()
        )
    }
}

impl<L: Dialect> std::error::Error for SpecializeError<L> {}

/// Error returned when [`Context::staged_function`] detects an existing
/// non-invalidated staged function with the same (name, signature).
///
/// The caller can either propagate this error or consume it via
/// [`Context::redefine_staged_function`] to intentionally invalidate the old
/// staged function and register the new one.
#[derive(Debug)]
pub struct StagedFunctionError<L: Dialect> {
    /// The conflicting interned name.
    pub name: Option<Symbol>,
    /// The conflicting signature.
    pub signature: Signature<L::Type>,
    /// Existing non-invalidated staged functions with the same (name, signature).
    pub conflicting: Vec<StagedFunction>,
    /// Preserved specializations for the new staged function.
    pub specializations: Vec<SpecializedFunctionInfo<L>>,
    /// Preserved backedges for the new staged function.
    pub backedges: Vec<StagedFunction>,
}

impl<L: Dialect> std::fmt::Display for StagedFunctionError<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "duplicate staged function: {} existing staged function(s) with the same (name, signature)",
            self.conflicting.len()
        )
    }
}

impl<L: Dialect> std::error::Error for StagedFunctionError<L> {}
