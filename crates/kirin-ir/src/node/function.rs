//! Three-level function refinement hierarchy.
//!
//! Functions in Kirin are organized into a three-level hierarchy that reflects
//! how programs evolve through compilation stages and specialization:
//!
//! ```text
//! Function              (stage-independent identity)
//!   └─ StagedFunction        (one per compilation stage)
//!       └─ SpecializedFunction    (one per signature specialization)
//! ```
//!
//! - [`Function`] / [`FunctionInfo`] — A logical function identity, independent
//!   of any compilation stage. It maps a name to the set of staged versions
//!   produced as the function moves through the pipeline.
//!
//! - [`StagedFunction`] / [`StagedFunctionInfo`] — A function compiled to a
//!   specific stage (e.g. parsed, optimized, lowered). Carries the generic
//!   signature and owns all specializations for that stage. Stages are *not*
//!   necessarily sequential — a user may program a low-level stage directly.
//!
//! - [`SpecializedFunction`] / [`SpecializedFunctionInfo`] — A concrete
//!   instantiation of a staged function for a particular (possibly narrower)
//!   signature. Owns the IR body. Dispatch selects the most specific
//!   non-invalidated specialization via [`SignatureSemantics`].
//!
//! Each level can be *invalidated* (staged or specialized) when the function is
//! redefined; invalidated entries are kept for backedge tracking but excluded
//! from dispatch and compilation.

use indexmap::IndexMap;

use crate::arena::{GetInfo, Id, Item};
use crate::language::Dialect;
use crate::signature::{Signature, SignatureCmp, SignatureSemantics};
use crate::{Statement, identifier};

use super::symbol::GlobalSymbol;

identifier! {
    /// A unique identifier for a compilation stage.
    ///
    /// Compilation stages represent different phases in the compilation pipeline,
    /// such as parsing, optimization, code generation, etc.
    /// Can be used as a compile-time value in statement definitions.
    struct CompileStage
}

impl CompileStage {
    pub fn new(stage: Id) -> Self {
        CompileStage(stage)
    }
}

identifier! {
    /// A unique identifier for a generic function.
    ///
    /// Functions can have multiple staged versions corresponding to different
    /// compilation stages.
    struct Function
}

identifier! {
    /// A unique identifier for a function at a specific compile stage.
    struct StagedFunction
}

/// Policy controlling whether multiple staged signatures can share the same function name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum StagedNamePolicy {
    /// Require a single semantic interface per function name.
    ///
    /// A new staged function with an existing name must have the same signature.
    #[default]
    SingleInterface,
    /// Allow multiple staged signatures under the same function name.
    ///
    /// Dispatch across signature variants is handled by signature semantics.
    MultipleDispatch,
}

/// A specialized version of a function, identified by its function ID and specialization ID.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct SpecializedFunction(pub(crate) StagedFunction, pub(crate) usize);

impl SpecializedFunction {
    pub fn id(&self) -> (StagedFunction, usize) {
        (self.0, self.1)
    }
}

/// Information about a function across different compilation stages.
#[derive(Clone, Debug)]
pub struct FunctionInfo {
    id: Function,
    name: Option<GlobalSymbol>,
    /// compiled versions of the function at different stages.
    ///
    /// note that compile stages may not be sequential,
    /// i.e., some stages may be skipped when a user directly programs a low-level stage
    /// language and modifies the compilation stage accordingly.
    ///
    /// Some early stages may be discarded later in the compilation pipeline to save memory.
    ///
    /// The execution will always look for the matching stage of the target execution environment
    /// e.g an interpreter will look for the staged function at the interpreter stage.
    /// but LLVM backend will look for the staged function at the LLVM IR generation stage.
    staged_functions: IndexMap<CompileStage, StagedFunction>,
}

impl FunctionInfo {
    pub fn new(id: Function, name: Option<GlobalSymbol>) -> Self {
        Self {
            id,
            name,
            staged_functions: IndexMap::new(),
        }
    }

    pub fn id(&self) -> Function {
        self.id
    }

    pub fn name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    pub fn staged_functions(&self) -> &IndexMap<CompileStage, StagedFunction> {
        &self.staged_functions
    }

    pub fn add_staged_function(&mut self, stage: CompileStage, func: StagedFunction) {
        self.staged_functions.insert(stage, func);
    }
}

/// A function compiled to a specific stage, carrying the generic signature
/// and all specializations for that stage.
///
/// Backedges track which other staged functions call this one (for
/// inter-procedural analyses). A staged function can be [`invalidated`](Self::invalidate)
/// when the source function is redefined; it is then retained for backedge
/// bookkeeping but excluded from new dispatch or compilation.
#[derive(Clone, Debug)]
pub struct StagedFunctionInfo<L: Dialect> {
    pub(crate) id: StagedFunction,
    pub(crate) name: Option<GlobalSymbol>,
    pub(crate) signature: Signature<L::Type>,
    pub(crate) specializations: Vec<SpecializedFunctionInfo<L>>,
    /// Functions that call this staged function (used for inter-procedural analyses).
    /// note that the call statement must refer to the `StagedFunction` ID,
    /// if it refers to a `SpecializedFunction`, it should be recorded in the
    /// `backedges` field of `SpecializedFunctionInfo`.
    /// thus the `backedges` field of `SpecializedFunctionInfo` is always a superset of this field.
    pub(crate) backedges: Vec<StagedFunction>,
    /// Whether this staged function has been invalidated by a redefinition.
    /// Invalidated staged functions are retained for backedge tracking but
    /// should not be considered for new dispatch or compilation.
    pub(crate) invalidated: bool,
}

impl<L: Dialect> StagedFunctionInfo<L> {
    pub fn id(&self) -> StagedFunction {
        self.id
    }

    pub fn name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    pub fn signature(&self) -> &Signature<L::Type> {
        &self.signature
    }

    pub fn return_type(&self) -> &L::Type {
        &self.signature.ret
    }

    pub fn backedges(&self) -> &Vec<StagedFunction> {
        &self.backedges
    }

    /// Returns whether this staged function has been invalidated by a redefinition.
    pub fn is_invalidated(&self) -> bool {
        self.invalidated
    }

    /// Mark this staged function as invalidated.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }

    /// Get the specializations of this staged function.
    /// The specialized function signature are strictly subset of the staged function signature
    pub fn specializations(&self) -> &Vec<SpecializedFunctionInfo<L>> {
        &self.specializations
    }

    pub fn specializations_mut(&mut self) -> &mut Vec<SpecializedFunctionInfo<L>> {
        &mut self.specializations
    }

    pub fn add_specialization(&mut self, spec: SpecializedFunctionInfo<L>) {
        self.specializations.push(spec);
    }

    /// Find all specializations applicable to the given call signature,
    /// reduced to the most specific candidates using the provided semantics.
    ///
    /// Invalidated specializations are excluded from matching.
    pub fn all_matching<S: SignatureSemantics<L::Type>>(
        &self,
        call: &Signature<L::Type>,
    ) -> Vec<(&SpecializedFunctionInfo<L>, S::Env)> {
        // Collect all applicable, non-invalidated specializations with their environments
        let applicable: Vec<_> = self
            .specializations
            .iter()
            .filter(|spec| !spec.is_invalidated())
            .filter_map(|spec| S::applicable(call, spec.signature()).map(|env| (spec, env)))
            .collect();

        // Reduce to the most specific candidates: keep only those where
        // no other applicable candidate is strictly more specific.
        applicable
            .into_iter()
            .filter(|(spec, env)| {
                !self
                    .specializations
                    .iter()
                    .filter(|other| !other.is_invalidated())
                    .any(|other| {
                        if std::ptr::eq(*spec, other) {
                            return false;
                        }
                        if let Some(other_env) = S::applicable(call, other.signature()) {
                            S::cmp_candidate(other.signature(), &other_env, spec.signature(), env)
                                == SignatureCmp::More
                        } else {
                            false
                        }
                    })
            })
            .collect()
    }
}

/// A concrete instantiation of a staged function for a specific signature.
///
/// The specialized signature is a subset of the parent [`StagedFunctionInfo`]'s
/// generic signature. This is the level that owns the IR [`body`](Self::body).
/// Like staged functions, specializations can be invalidated and are then
/// excluded from dispatch while remaining available for backedge tracking.
#[derive(Clone, Debug)]
pub struct SpecializedFunctionInfo<L: Dialect> {
    id: SpecializedFunction,
    signature: Signature<L::Type>,
    body: Statement,
    /// Functions that call this function (used for inter-procedural analyses).
    backedges: Vec<SpecializedFunction>,
    /// Whether this specialization has been invalidated by a redefinition.
    /// Invalidated specializations are retained for backedge tracking but
    /// should not be matched during dispatch.
    invalidated: bool,
}

#[bon::bon]
impl<L: Dialect> SpecializedFunctionInfo<L> {
    #[builder(finish_fn = new)]
    pub fn new(
        /// The unique identifier for this specialized function.
        id: SpecializedFunction,
        /// The signature of this specialized function.
        signature: Signature<L::Type>,
        /// The body of this specialized function.
        body: Statement,
        /// The functions that call this specialized function.
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> Self {
        Self {
            id,
            signature,
            body,
            backedges: backedges.unwrap_or_default(),
            invalidated: false,
        }
    }
}

impl From<FunctionInfo> for Function {
    fn from(fi: FunctionInfo) -> Self {
        fi.id
    }
}

impl<L: Dialect> From<StagedFunctionInfo<L>> for StagedFunction {
    fn from(sfi: StagedFunctionInfo<L>) -> Self {
        sfi.id
    }
}

impl<L: Dialect> From<SpecializedFunctionInfo<L>> for SpecializedFunction {
    fn from(sfi: SpecializedFunctionInfo<L>) -> Self {
        sfi.id
    }
}

impl<L: Dialect> SpecializedFunctionInfo<L> {
    pub fn id(&self) -> SpecializedFunction {
        self.id
    }

    pub fn body(&self) -> &Statement {
        &self.body
    }

    pub fn body_mut(&mut self) -> &mut Statement {
        &mut self.body
    }

    pub fn return_type(&self) -> &L::Type {
        &self.signature.ret
    }

    pub fn signature(&self) -> &Signature<L::Type> {
        &self.signature
    }

    pub fn backedges(&self) -> &Vec<SpecializedFunction> {
        &self.backedges
    }

    /// Returns whether this specialization has been invalidated by a redefinition.
    pub fn is_invalidated(&self) -> bool {
        self.invalidated
    }

    /// Mark this specialization as invalidated.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }
}

impl<L: Dialect> GetInfo<L> for StagedFunction {
    type Info = Item<StagedFunctionInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.staged_functions.get(*self)
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.staged_functions.get_mut(*self)
    }
}

impl<L: Dialect> GetInfo<L> for SpecializedFunction {
    type Info = SpecializedFunctionInfo<L>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        let (staged_func, idx) = self.id();
        stage
            .staged_functions
            .get(staged_func)
            .and_then(|f| f.specializations.get(idx))
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        let (staged_func, idx) = self.id();
        stage
            .staged_functions
            .get_mut(staged_func)
            .and_then(|f| f.specializations.get_mut(idx))
    }
}
