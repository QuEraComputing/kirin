use kirin_ir::{Product, ResultValue};
use smallvec::SmallVec;

use crate::{InterpreterError, ValueStore};

/// Interpreter-level product value semantics.
///
/// Uses the same `Product<T>` from kirin-ir. 2 required methods,
/// all operations provided. No unnecessary allocation.
pub trait ProductValue: Sized + Clone {
    /// Borrow the product storage if this value is a product.
    fn as_product(&self) -> Option<&Product<Self>>;

    /// Wrap a product into this value type.
    fn from_product(product: Product<Self>) -> Self;

    // --- All provided ---

    /// Pack multiple values into a product value.
    fn new_product(values: Vec<Self>) -> Self {
        Self::from_product(Product(SmallVec::from_vec(values)))
    }

    /// Extract one element by index (clones the element).
    fn get(&self, index: usize) -> Result<Self, InterpreterError> {
        self.as_product()
            .and_then(|p| p.get(index).cloned())
            .ok_or_else(|| {
                InterpreterError::Custom(format!("product index {index} out of bounds").into())
            })
    }

    /// Query the number of elements.
    fn len(&self) -> Result<usize, InterpreterError> {
        self.as_product()
            .map(|p| p.len())
            .ok_or_else(|| InterpreterError::Custom("expected product".into()))
    }

    /// Returns true if the product has zero elements.
    fn is_empty(&self) -> Result<bool, InterpreterError> {
        self.len().map(|n| n == 0)
    }
}

/// Trivial impl for `i64`: not a product type, so `as_product` always
/// returns `None`. Used by concrete interpreter tests.
impl ProductValue for i64 {
    fn as_product(&self) -> Option<&Product<Self>> {
        None
    }

    fn from_product(product: Product<Self>) -> Self {
        assert_eq!(
            product.len(),
            1,
            "i64 cannot represent a product of {} elements",
            product.len()
        );
        product[0]
    }
}

/// Auto-destructure a single value into multiple result slots.
///
/// If `results` has 0 or 1 entries, writes directly (no product overhead).
/// If `results` has N > 1 entries, treats `value` as a product and writes
/// each element to the corresponding result slot.
pub fn write_statement_results<S>(
    store: &mut S,
    results: &[ResultValue],
    value: S::Value,
) -> Result<(), S::Error>
where
    S: ValueStore,
    S::Value: ProductValue,
    S::Error: From<InterpreterError>,
{
    match results.len() {
        0 => Ok(()),
        1 => store.write(results[0], value),
        _ => {
            for (i, rv) in results.iter().enumerate() {
                let element = ProductValue::get(&value, i).map_err(S::Error::from)?;
                store.write(*rv, element)?;
            }
            Ok(())
        }
    }
}
