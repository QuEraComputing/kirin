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
///     fn signature(&self, stage: &StageInfo<Circuit>) -> Option<Signature<QubitType>> {
///         let info = self.body.expect_info(stage);
///         let params: Vec<QubitType> = info
///             .edge_ports()
///             .iter()
///             .map(|p| p.expect_info(stage).ty().clone())
///             .collect();
///         let ret = QubitType::Qubit;
///         Some(Signature::new(params, ret, ()))
///     }
/// }
/// ```
pub trait HasSignature<L: Dialect> {
    /// Returns the function signature extracted from this statement, or `None`
    /// if the type does not carry a signature (e.g. non-function-body statements).
    fn signature(&self, stage: &StageInfo<L>) -> Option<Signature<L::Type>>;
}
