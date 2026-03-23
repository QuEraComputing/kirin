use kirin_ir::{ResultValue, SSAValue};

use crate::{InterpreterError, ProductValue};

/// Value read/write operations for SSA bindings.
///
/// This is the minimal storage interface that all interpreter implementations
/// share. Dialect `Interpretable` impls use this to read operands and write
/// results.
pub trait ValueStore {
    /// The value type manipulated by this interpreter.
    ///
    /// Values should be cheap to clone — typically pointer-sized handles,
    /// small enums, or wrappers around `Arc`/`Rc` for heavier data.
    type Value: Clone;
    type Error;

    /// Returns a cloned copy of the bound value.
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;

    /// Read multiple SSA values into a `Vec`.
    fn read_many(&self, values: &[SSAValue]) -> Result<Vec<Self::Value>, Self::Error> {
        values.iter().map(|ssa| self.read(*ssa)).collect()
    }

    /// Bind a value to an SSA slot.
    ///
    /// Accepts `ResultValue`, `BlockArgument`, `SSAValue`, etc. — anything
    /// that converts to `SSAValue` via `Into`.
    fn write(&mut self, target: impl Into<SSAValue>, value: Self::Value)
    -> Result<(), Self::Error>;

    /// Bind multiple results to values with arity checking.
    ///
    /// Returns [`InterpreterError::ArityMismatch`] if the number of values
    /// does not match the number of result slots.
    fn write_many(
        &mut self,
        results: &[ResultValue],
        values: &[Self::Value],
    ) -> Result<(), Self::Error>
    where
        Self::Error: From<InterpreterError>,
    {
        if results.len() != values.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: results.len(),
                got: values.len(),
            }
            .into());
        }
        for (rv, val) in results.iter().zip(values.iter()) {
            self.write(SSAValue::from(*rv), val.clone())?;
        }
        Ok(())
    }

    /// Auto-destructure a single value into multiple result slots.
    ///
    /// If `results` has 0 or 1 entries, writes directly (no product overhead).
    /// If `results` has N > 1 entries, treats `value` as a product and writes
    /// each element to the corresponding result slot.
    fn write_product(
        &mut self,
        results: &[ResultValue],
        value: Self::Value,
    ) -> Result<(), Self::Error>
    where
        Self::Value: ProductValue,
        Self::Error: From<InterpreterError>,
    {
        match results.len() {
            0 => Ok(()),
            1 => self.write(SSAValue::from(results[0]), value),
            _ => {
                for (i, rv) in results.iter().enumerate() {
                    let element = ProductValue::get(&value, i).map_err(Self::Error::from)?;
                    self.write(SSAValue::from(*rv), element)?;
                }
                Ok(())
            }
        }
    }
}
