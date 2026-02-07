use crate::language::Dialect;
use crate::node::function::{SpecializedFunction, SpecializedFunctionInfo, StagedFunction};
use crate::node::stmt::Statement;
use crate::node::symbol::GlobalSymbol;
use crate::signature::Signature;

/// Why staged function creation conflicted with existing definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedFunctionConflictKind {
    /// A non-invalidated staged function already exists with the same (name, signature).
    DuplicateSignature,
    /// Name is already in use with a different signature while `SingleInterface` is active.
    SignatureMismatchUnderSingleInterface,
}

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
/// non-invalidated staged function with the same name.
///
/// This catches both exact duplicates (same name + same signature) and
/// signature conflicts (same name + different signature while
/// `SingleInterface` policy is active). Inspect `conflict_kind` to
/// distinguish the cases.
///
/// The caller can either propagate this error or consume it via
/// [`Context::redefine_staged_function`] to intentionally invalidate the old
/// staged function and register the new one.
#[derive(Debug)]
pub struct StagedFunctionError<L: Dialect> {
    /// Why this staged-function creation conflicted.
    pub conflict_kind: StagedFunctionConflictKind,
    /// The conflicting global symbol name.
    pub name: Option<GlobalSymbol>,
    /// The signature of the new staged function being created.
    pub signature: Signature<L::Type>,
    /// Existing non-invalidated staged functions with the same name.
    pub conflicting: Vec<StagedFunction>,
    /// Preserved specializations for the new staged function.
    pub specializations: Vec<SpecializedFunctionInfo<L>>,
    /// Preserved backedges for the new staged function.
    pub backedges: Vec<StagedFunction>,
}

impl<L: Dialect> std::fmt::Display for StagedFunctionError<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.conflict_kind {
            StagedFunctionConflictKind::DuplicateSignature => write!(
                f,
                "duplicate staged function: {} existing staged function(s) with the same (name, signature)",
                self.conflicting.len()
            ),
            StagedFunctionConflictKind::SignatureMismatchUnderSingleInterface => write!(
                f,
                "staged function signature mismatch: {} existing staged function(s) share the name but have a different signature under SingleInterface policy",
                self.conflicting.len()
            ),
        }
    }
}

impl<L: Dialect> std::error::Error for StagedFunctionError<L> {}
