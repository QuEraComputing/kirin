use kirin_ir::Product;

use crate::InterpreterError;

/// Values that can drive conditional control flow.
///
/// `None` means the condition is undecided in the value domain (e.g. a
/// lattice `Top`); dialects then emit the undecided effect variants.
pub trait BranchCondition {
    fn is_truthy(&self) -> Option<bool>;
}

impl BranchCondition for i64 {
    fn is_truthy(&self) -> Option<bool> {
        Some(*self != 0)
    }
}

/// Value domains that expose an explicit tuple/product runtime value
/// (used by the tuple dialect).
pub trait HasProductValue: Clone + Sized {
    fn from_product(product: Product<Self>) -> Self;
    fn as_product(&self) -> Option<&Product<Self>>;
}

/// Extract the single value from a function-return product.
pub fn expect_single<V, E>(product: Product<V>) -> Result<V, E>
where
    E: From<InterpreterError>,
{
    if product.len() != 1 {
        return Err(E::from(InterpreterError::ExpectedSingleReturn(
            product.len(),
        )));
    }
    Ok(product.into_iter().next().unwrap())
}
