use crate::Dialect;

use super::Signature;

/// Extract the function signature from a parsed function-body statement.
///
/// Implemented by dialect types that serve as function bodies (e.g., `FunctionBody`,
/// `CircuitFunction`). The framework calls this after parsing to construct the
/// `SpecializedFunction`.
///
/// With RFC 0004, the signature is a field on the statement type — `derive(Dialect)`
/// generates this trait automatically. Types with a `Signature<T>` field return
/// `Some(sig.clone())`; types without return `None`.
///
/// # Type parameters
///
/// - `L`: The dialect whose `Type` is used in the signature.
pub trait HasSignature<L: Dialect> {
    /// Returns the function signature from this statement, or `None`
    /// if the type does not carry a signature (e.g. non-function-body statements).
    fn signature(&self) -> Option<Signature<L::Type>>;
}
