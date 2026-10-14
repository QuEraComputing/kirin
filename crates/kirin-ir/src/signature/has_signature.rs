use crate::Dialect;

use super::Signature;

/// Extract the function signature from a parsed function-definition statement.
///
/// Implemented by dialect types that serve as function definitions (e.g.,
/// `Function`, `CircuitFunction`). The framework calls this after parsing to
/// construct the `SpecializedFunction`.
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
    /// if the type does not carry a signature (e.g. non-definition statements).
    fn signature(&self) -> Option<Signature<L::Type>>;
}
