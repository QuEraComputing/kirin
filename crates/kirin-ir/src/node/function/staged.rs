use crate::arena::{GetInfo, Id, Item};
use crate::identifier;
use crate::language::Dialect;
use crate::signature::{Signature, SignatureCmp, SignatureSemantics};

use super::specialized::SpecializedFunctionInfo;
use crate::node::symbol::GlobalSymbol;

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

    pub fn backedges(&self) -> &[StagedFunction] {
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
    pub fn specializations(&self) -> &[SpecializedFunctionInfo<L>] {
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
        let dominated: Vec<bool> = applicable
            .iter()
            .enumerate()
            .map(|(i, (spec, env))| {
                applicable
                    .iter()
                    .enumerate()
                    .any(|(j, (other, other_env))| {
                        i != j
                            && S::cmp_candidate(other.signature(), other_env, spec.signature(), env)
                                == SignatureCmp::More
                    })
            })
            .collect();

        applicable
            .into_iter()
            .zip(dominated)
            .filter(|(_, d)| !*d)
            .map(|(item, _)| item)
            .collect()
    }
}

impl<L: Dialect> From<StagedFunctionInfo<L>> for StagedFunction {
    fn from(sfi: StagedFunctionInfo<L>) -> Self {
        sfi.id
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
