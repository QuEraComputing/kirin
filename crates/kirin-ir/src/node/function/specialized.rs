use crate::Statement;
use crate::arena::GetInfo;
use crate::language::Dialect;
use crate::signature::Signature;

use super::staged::StagedFunction;

/// A specialized version of a function, identified by its function ID and specialization ID.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct SpecializedFunction(pub(crate) StagedFunction, pub(crate) usize);

impl SpecializedFunction {
    pub fn id(&self) -> (StagedFunction, usize) {
        (self.0, self.1)
    }
}

/// A concrete instantiation of a staged function for a specific signature.
///
/// The specialized signature is a subset of the parent [`StagedFunctionInfo`](super::staged::StagedFunctionInfo)'s
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
