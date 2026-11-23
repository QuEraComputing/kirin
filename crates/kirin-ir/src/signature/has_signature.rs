use crate::Dialect;

use super::Signature;

/// The signature a function-definition statement declares.
///
/// Implemented by dialect types that serve as function definitions (e.g.,
/// `Function`, `CircuitFunction`). `derive(Dialect)` generates it: a type with
/// a `Signature<T>` field returns that field, a `#[wraps]` type delegates, and
/// any other type returns `None`.
///
/// The definition statement is the single owner of a specialization's
/// signature. [`SpecializedFunctionInfo`](crate::SpecializedFunctionInfo)
/// stores none and reads it from here, so a statement that returns `None`
/// cannot be a specialization's definition. The interface a function exposes
/// at a stage is a separate fact, owned by
/// [`StagedFunctionInfo`](crate::StagedFunctionInfo).
///
/// # Type parameters
///
/// - `L`: The dialect whose `Type` is used in the signature.
pub trait HasSignature<L: Dialect> {
    /// Returns the signature this statement declares, or `None` if it
    /// declares none (e.g. non-definition statements).
    fn signature(&self) -> Option<&Signature<L::Type>>;
}
