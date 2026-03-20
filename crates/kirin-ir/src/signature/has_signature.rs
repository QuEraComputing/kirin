use crate::{Dialect, StageInfo};

use super::Signature;

/// Extract the function signature from a parsed function-body statement.
///
/// Implemented by dialect types that serve as function bodies (e.g., `CircuitFunction`,
/// `FunctionBody`). The framework calls this after parsing to construct the
/// `SpecializedFunction`.
///
/// # Type parameters
///
/// - `L`: The dialect whose `Type` is used in the signature.
///
/// # Examples
///
/// ```ignore
/// impl HasSignature<Circuit> for CircuitFunction {
///     fn signature(&self, stage: &StageInfo<Circuit>) -> Signature<QubitType> {
///         // Extract params from graph ports, return type from yields
///         todo!()
///     }
/// }
/// ```
pub trait HasSignature<L: Dialect> {
    /// Returns the function signature extracted from this statement.
    fn signature(&self, stage: &StageInfo<L>) -> Signature<L::Type>;
}
