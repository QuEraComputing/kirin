use crate::arena::{GetInfo, GetInfoMut};
use crate::language::Dialect;
use crate::signature::{HasSignature, Signature};
use crate::{Product, Statement};

use super::staged::StagedFunction;

/// A specialized version of a function, identified by its function ID and specialization ID.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecializedFunction(pub(crate) StagedFunction, pub(crate) usize);

impl SpecializedFunction {
    pub fn id(&self) -> (StagedFunction, usize) {
        (self.0, self.1)
    }
}

/// A concrete instantiation of a staged function for a specific signature.
///
/// This is the level that owns the IR [`definition`](Self::definition). It
/// stores no signature of its own: the definition statement declares it (see
/// [`HasSignature`]), and [`signature`](Self::signature) reads it from there.
/// That signature has the shape of the parent
/// [`StagedFunctionInfo`](super::staged::StagedFunctionInfo)'s interface and
/// is expected to refine it type by type.
/// Like staged functions, specializations can be invalidated and are then
/// excluded from dispatch while remaining available for backedge tracking.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecializedFunctionInfo {
    id: SpecializedFunction,
    definition: Statement,
    /// Functions that call this function (used for inter-procedural analyses).
    backedges: Vec<SpecializedFunction>,
    /// Whether this specialization has been invalidated by a redefinition.
    /// Invalidated specializations are retained for backedge tracking but
    /// should not be matched during dispatch.
    invalidated: bool,
}

#[bon::bon]
impl SpecializedFunctionInfo {
    #[builder(finish_fn = new)]
    pub fn new(
        /// The unique identifier for this specialized function.
        id: SpecializedFunction,
        /// The definition statement of this specialized function.
        definition: Statement,
        /// The functions that call this specialized function.
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> Self {
        Self {
            id,
            definition,
            backedges: backedges.unwrap_or_default(),
            invalidated: false,
        }
    }
}

impl From<SpecializedFunctionInfo> for SpecializedFunction {
    fn from(sfi: SpecializedFunctionInfo) -> Self {
        sfi.id
    }
}

impl SpecializedFunctionInfo {
    pub fn id(&self) -> SpecializedFunction {
        self.id
    }

    pub fn definition(&self) -> &Statement {
        &self.definition
    }

    /// The signature of this specialization, read from its definition
    /// statement, which is its single owner.
    ///
    /// A specialization is only created from a definition that declares a
    /// signature, so this is `None` only if that statement has since been
    /// removed from `stage`.
    pub fn signature<'a, L>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Signature<L::Type>>
    where
        L: Dialect + HasSignature<L>,
    {
        self.definition.get_info(stage)?.definition().signature()
    }

    /// The return types of [`signature`](Self::signature).
    pub fn return_types<'a, L>(
        &self,
        stage: &'a crate::StageInfo<L>,
    ) -> Option<&'a Product<L::Type>>
    where
        L: Dialect + HasSignature<L>,
    {
        self.signature(stage).map(Signature::rets)
    }

    pub fn backedges(&self) -> &[SpecializedFunction] {
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

    pub(crate) fn add_backedge(&mut self, caller: SpecializedFunction) {
        if !self.backedges.contains(&caller) {
            self.backedges.push(caller);
        }
    }
}

impl<L: Dialect> GetInfo<L> for SpecializedFunction {
    type Info = SpecializedFunctionInfo;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        let (staged_func, idx) = self.id();
        stage
            .staged_functions
            .get(staged_func)
            .and_then(|f| f.specializations.get(idx))
    }
}

impl<L: Dialect> GetInfoMut<L> for SpecializedFunction {
    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        let (staged_func, idx) = self.id();
        stage
            .staged_functions
            .get_mut(staged_func)
            .and_then(|f| f.specializations.get_mut(idx))
    }
}
