use crate::language::Dialect;
use crate::node::function::{Function, SpecializedFunction, StagedFunction};
use crate::node::stmt::Statement;
use crate::node::symbol::GlobalSymbol;
use crate::signature::Signature;

/// Error returned by [`crate::Program`] mutation methods that previously panicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramError {
    /// The [`Function`] ID does not exist in the program's arena.
    UnknownFunction(Function),
    /// A function with the same name has already been allocated.
    DuplicateFunctionName(GlobalSymbol),
}

impl std::fmt::Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgramError::UnknownFunction(func) => write!(f, "unknown function: {func:?}"),
            ProgramError::DuplicateFunctionName(sym) => {
                write!(f, "duplicate function name: {sym:?}")
            }
        }
    }
}

impl std::error::Error for ProgramError {}

/// Error returned by [`crate::Program::staged_function`] and
/// [`crate::Program::define_function`], which can fail either at the
/// program level ([`ProgramError`]) or at the stage level
/// ([`StagedFunctionError`]).
#[derive(Debug)]
pub enum ProgramStagedError<L: Dialect> {
    /// A program-level error (unknown function, invalid stage, etc.).
    Program(ProgramError),
    /// A stage-level conflict when creating the staged function.
    StagedFunction(StagedFunctionError<L>),
    /// The definition cannot specialize the staged function.
    Specialize(SpecializeError),
}

impl<L: Dialect> std::fmt::Display for ProgramStagedError<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgramStagedError::Program(e) => e.fmt(f),
            ProgramStagedError::StagedFunction(e) => e.fmt(f),
            ProgramStagedError::Specialize(e) => e.fmt(f),
        }
    }
}

impl<L: Dialect> std::error::Error for ProgramStagedError<L> {}

impl<L: Dialect> From<ProgramError> for ProgramStagedError<L> {
    fn from(e: ProgramError) -> Self {
        ProgramStagedError::Program(e)
    }
}

impl<L: Dialect> From<SpecializeError> for ProgramStagedError<L> {
    fn from(e: SpecializeError) -> Self {
        ProgramStagedError::Specialize(e)
    }
}

impl<L: Dialect> From<StagedFunctionError<L>> for ProgramStagedError<L> {
    fn from(e: StagedFunctionError<L>) -> Self {
        ProgramStagedError::StagedFunction(e)
    }
}

/// Why staged function creation conflicted with existing definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedFunctionConflictKind {
    /// A non-invalidated staged function already exists with the same (name, signature).
    DuplicateSignature,
    /// Name is already in use with a different signature while `SingleInterface` is active.
    SignatureMismatchUnderSingleInterface,
}

/// Error returned by [`crate::StageInfo::specialize`].
#[derive(Debug)]
pub enum SpecializeError {
    /// A non-invalidated specialization already has the signature. It can be
    /// replaced through [`crate::StageInfo::redefine_specialization`].
    Duplicate(DuplicateSpecialization),
    /// The staged function has been invalidated by a redefinition, so it
    /// takes no new specializations.
    InvalidatedStagedFunction(StagedFunction),
    /// The definition is not a statement of this stage.
    UnknownDefinition(Statement),
    /// The definition statement declares no signature. The definition is the
    /// single owner of a specialization's signature, so a statement without
    /// one cannot define a specialization.
    MissingSignature(Statement),
    /// The definition's signature has a different parameter or return count
    /// than the staged function's interface.
    InterfaceMismatch(InterfaceMismatch),
    /// The definition's signature has the interface's shape but does not
    /// refine it under the dialect's
    /// [`Signatures`](crate::Dialect::Signatures) (see
    /// [`SignatureSemantics::refines`](crate::SignatureSemantics::refines)).
    NotARefinement {
        /// The staged function being specialized.
        staged_function: StagedFunction,
        /// The rejected definition statement.
        definition: Statement,
    },
}

/// A specialization whose signature does not have the shape of its staged
/// function's interface. Each pair is `(interface, specialization)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceMismatch {
    /// The staged function being specialized.
    pub staged_function: StagedFunction,
    /// The rejected definition statement.
    pub definition: Statement,
    /// Parameter counts.
    pub params: (usize, usize),
    /// Return counts.
    pub rets: (usize, usize),
}

impl std::fmt::Display for InterfaceMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "specialization does not fit its interface: the interface takes {} parameter(s) and returns {} value(s), the definition takes {} and returns {}",
            self.params.0, self.rets.0, self.params.1, self.rets.1
        )
    }
}

impl std::fmt::Display for SpecializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecializeError::Duplicate(e) => e.fmt(f),
            SpecializeError::InvalidatedStagedFunction(func) => {
                write!(f, "cannot specialize invalidated staged function: {func:?}")
            }
            SpecializeError::UnknownDefinition(stmt) => {
                write!(f, "definition is not a statement of this stage: {stmt:?}")
            }
            SpecializeError::MissingSignature(_) => write!(
                f,
                "definition declares no signature: a function definition must carry a `Signature` field"
            ),
            SpecializeError::InterfaceMismatch(e) => e.fmt(f),
            SpecializeError::NotARefinement { .. } => write!(
                f,
                "specialization does not fit its interface: its signature does not refine the staged function's signature"
            ),
        }
    }
}

impl std::error::Error for SpecializeError {}

/// Error returned by [`crate::BuilderStageInfo::invalidate_specialization`]
/// and [`crate::BuilderStageInfo::add_specialization_backedge`]: the
/// specialization does not exist in this stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownSpecialization(pub SpecializedFunction);

impl std::fmt::Display for UnknownSpecialization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown specialization: {:?}", self.0)
    }
}

impl std::error::Error for UnknownSpecialization {}

/// An existing non-invalidated specialization with the same signature, found
/// by [`crate::StageInfo::specialize`].
///
/// The caller can either propagate this error or consume it via
/// [`crate::StageInfo::redefine_specialization`] to intentionally invalidate the old
/// specialization and register the new one.
#[derive(Debug)]
pub struct DuplicateSpecialization {
    /// The staged function being specialized.
    pub staged_function: StagedFunction,
    /// Existing non-invalidated specializations with matching signatures.
    pub conflicting: Vec<SpecializedFunction>,
    /// Preserved definition statement for the new specialization.
    pub definition: Statement,
    /// Preserved backedges for the new specialization.
    pub backedges: Option<Vec<SpecializedFunction>>,
}

impl std::fmt::Display for DuplicateSpecialization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "duplicate specialization: {} existing specialization(s) with the same signature",
            self.conflicting.len()
        )
    }
}

impl std::error::Error for DuplicateSpecialization {}

/// Error returned when [`crate::StageInfo::staged_function`] detects an existing
/// non-invalidated staged function with the same name.
///
/// This catches both exact duplicates (same name + same signature) and
/// signature conflicts (same name + different signature while
/// `SingleInterface` policy is active). Inspect `conflict_kind` to
/// distinguish the cases.
///
/// The caller can either propagate this error or consume it via
/// [`crate::StageInfo::redefine_staged_function`] to intentionally invalidate the old
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
