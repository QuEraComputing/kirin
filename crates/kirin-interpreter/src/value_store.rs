use kirin_ir::{ResultValue, SSAValue};
use smallvec::SmallVec;

use crate::InterpreterError;

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

    /// Read multiple SSA values into a `SmallVec`.
    fn read_many(&self, values: &[SSAValue]) -> Result<SmallVec<[Self::Value; 1]>, Self::Error> {
        values.iter().map(|ssa| self.read(*ssa)).collect()
    }

    /// Bind a single result to a value.
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;

    /// Bind multiple results to values with arity checking.
    ///
    /// Returns [`InterpreterError::ArityMismatch`] if the number of values
    /// does not match the number of result slots.
    fn write_many(
        &mut self,
        results: &[ResultValue],
        values: &SmallVec<[Self::Value; 1]>,
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
            self.write(*rv, val.clone())?;
        }
        Ok(())
    }

    /// Bind an SSA value directly (e.g. block arguments).
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;
}
