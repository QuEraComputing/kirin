use std::hash::Hash;

use super::{AnalysisError, Database};

/// Names an analysis, the key it is asked with, and the type of its result.
///
/// The type this is implemented for is the *analysis type*: it identifies the
/// analysis and is usually an empty struct, such as `struct ConstPropFacts;`.
/// Two analyses can share a key type and a result type, and the cache tells
/// them apart by their analysis types.
///
/// The trait is not generic over the program's stage enum `S` (a key may
/// still contain `CompileStage` ids), so a rule set, which knows only its
/// language, can name an analysis ([`Requirement::of`](super::Requirement::of))
/// and read its result ([`AnalysisCache::get`](super::AnalysisCache::get)).
/// Computing it needs `S`: see [`Analysis`].
pub trait AnalysisKind: 'static {
    /// What one result is for, such as a stage or a set of roots. Each
    /// distinct key gets its own result.
    type Key: Clone + Eq + Hash + Send + Sync + 'static;
    /// The result for one key.
    type Output: Send + Sync + 'static;
}

/// How to compute analysis `Self` on a `Program<S>`.
pub trait Analysis<S>: AnalysisKind {
    /// Compute the result for `key`. `db` is shared, as a salsa query gets
    /// `&db`: the IR is read through [`Database::program`], and the analyses
    /// this one depends on through [`Database::get`]. An analysis whose key
    /// names a stage the program no longer holds returns
    /// [`AnalysisError::UnknownStage`].
    fn run(db: &Database<S>, key: &Self::Key) -> Result<Self::Output, AnalysisError>;
}
