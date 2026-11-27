use crate::Product;
use crate::arena::{GetInfo, GetInfoMut, Item};
use crate::identifier;
use crate::language::Dialect;
use crate::signature::{HasSignature, Signature, SignatureCmp, SignatureSemantics};

use super::specialized::{SpecializedFunction, SpecializedFunctionInfo};
use crate::node::symbol::GlobalSymbol;

identifier! {
    /// A unique identifier for a function at a specific compile stage.
    struct StagedFunction
}

/// Policy controlling whether multiple staged signatures can share the same function name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniqueLiveSpecializationError {
    NoSpecialization,
    Ambiguous { count: usize },
}

impl std::fmt::Display for UniqueLiveSpecializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSpecialization => write!(f, "no live specialization"),
            Self::Ambiguous { count } => {
                write!(f, "ambiguous live specialization set: {count} live matches")
            }
        }
    }
}

impl std::error::Error for UniqueLiveSpecializationError {}

/// What the dialect's [`Signatures`](Dialect::Signatures) produces for an
/// applicable candidate.
type SignatureEnv<L> =
    <<L as Dialect>::Signatures as SignatureSemantics<<L as Dialect>::Type>>::Env;

/// A function compiled to a specific stage, carrying the generic signature
/// and all specializations for that stage.
///
/// Backedges track which other staged functions call this one (for
/// inter-procedural analyses). A staged function can be [`invalidated`](Self::invalidate)
/// when the source function is redefined; it is then retained for backedge
/// bookkeeping but excluded from new dispatch or compilation.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(
        serialize = "L::Type: serde::Serialize",
        deserialize = "L::Type: serde::Deserialize<'de>"
    ))
)]
pub struct StagedFunctionInfo<L: Dialect> {
    pub(crate) id: StagedFunction,
    pub(crate) name: Option<GlobalSymbol>,
    pub(crate) signature: Signature<L::Type>,
    pub(crate) specializations: Vec<SpecializedFunctionInfo>,
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

    pub fn return_types(&self) -> &Product<L::Type> {
        self.signature.rets()
    }

    pub fn backedges(&self) -> &[StagedFunction] {
        &self.backedges
    }

    /// Returns whether this staged function has been invalidated by a redefinition.
    pub fn is_invalidated(&self) -> bool {
        self.invalidated
    }

    /// Mark this staged function, and with it every specialization, as
    /// invalidated.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
        self.specializations
            .iter_mut()
            .for_each(SpecializedFunctionInfo::invalidate);
    }

    /// Get the specializations of this staged function.
    ///
    /// Each specialization's signature has the same shape as this staged
    /// function's interface signature (checked when it is created) and is
    /// expected to refine it type by type (see
    /// [`SignatureSemantics::refines`]).
    pub fn specializations(&self) -> &[SpecializedFunctionInfo] {
        &self.specializations
    }

    pub fn unique_live_specialization(
        &self,
    ) -> Result<SpecializedFunction, UniqueLiveSpecializationError> {
        let mut live_specializations = self
            .specializations
            .iter()
            .filter(|spec| !spec.is_invalidated())
            .map(|spec| spec.id());

        match (live_specializations.next(), live_specializations.next()) {
            (None, _) => Err(UniqueLiveSpecializationError::NoSpecialization),
            (Some(callee), None) => Ok(callee),
            (Some(_), Some(_)) => Err(UniqueLiveSpecializationError::Ambiguous {
                count: self
                    .specializations
                    .iter()
                    .filter(|spec| !spec.is_invalidated())
                    .count(),
            }),
        }
    }

    /// Find all specializations applicable to the given call signature,
    /// reduced to the most specific candidates under the dialect's
    /// [`Signatures`](Dialect::Signatures).
    ///
    /// Invalidated specializations are excluded from matching.
    pub fn all_matching(
        &self,
        stage: &crate::StageInfo<L>,
        call: &Signature<L::Type>,
    ) -> Vec<(&SpecializedFunctionInfo, SignatureEnv<L>)>
    where
        L: HasSignature<L>,
    {
        // Collect all applicable, non-invalidated specializations with their environments
        let applicable: Vec<_> = self
            .specializations
            .iter()
            .filter(|spec| !spec.is_invalidated())
            .filter_map(|spec| {
                let signature = spec.signature(stage)?;
                L::Signatures::applicable(call, signature).map(|env| (spec, signature, env))
            })
            .collect();

        // Reduce to the most specific candidates: keep only those where
        // no other applicable candidate is strictly more specific.
        let dominated: Vec<bool> = applicable
            .iter()
            .enumerate()
            .map(|(i, (_, signature, env))| {
                applicable
                    .iter()
                    .enumerate()
                    .any(|(j, (_, other, other_env))| {
                        i != j
                            && L::Signatures::cmp_candidate(other, other_env, signature, env)
                                == SignatureCmp::More
                    })
            })
            .collect();

        applicable
            .into_iter()
            .zip(dominated)
            .filter(|(_, d)| !*d)
            .map(|((spec, _, env), _)| (spec, env))
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
}

impl<L: Dialect> GetInfoMut<L> for StagedFunction {
    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.staged_functions.get_mut(*self)
    }
}
