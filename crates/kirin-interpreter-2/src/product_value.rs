use kirin_ir::Product;
use smallvec::SmallVec;

use crate::InterpreterError;

/// Interpreter-level product value semantics.
pub trait ProductValue: Sized + Clone {
    fn as_product(&self) -> Option<&Product<Self>>;

    fn from_product(product: Product<Self>) -> Self;

    fn new_product(values: Vec<Self>) -> Self {
        match values.len() {
            1 => values.into_iter().next().unwrap(),
            _ => Self::from_product(Product(SmallVec::from_vec(values))),
        }
    }

    fn get(&self, index: usize) -> Result<Self, InterpreterError> {
        self.as_product()
            .and_then(|product| product.get(index).cloned())
            .ok_or_else(|| {
                InterpreterError::message(format!("product index {index} out of bounds"))
            })
    }

    fn len(&self) -> Result<usize, InterpreterError> {
        self.as_product()
            .map(Product::len)
            .ok_or_else(|| InterpreterError::message("expected product"))
    }

    fn is_empty(&self) -> Result<bool, InterpreterError> {
        self.len().map(|len| len == 0)
    }
}
