use kirin_ir::{ResultValue, SSAValue};

use crate::{InterpreterError, ProductValue};

/// Typed SSA read/write storage used by stage-local shells.
pub trait ValueStore {
    type Value: Clone;
    type Error;

    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;

    fn read_many(&self, values: &[SSAValue]) -> Result<Vec<Self::Value>, Self::Error> {
        values.iter().map(|ssa| self.read(*ssa)).collect()
    }

    fn write(&mut self, target: impl Into<SSAValue>, value: Self::Value)
    -> Result<(), Self::Error>;

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

        for (result, value) in results.iter().zip(values.iter()) {
            self.write(*result, value.clone())?;
        }

        Ok(())
    }

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
                for (index, result) in results.iter().enumerate() {
                    let element = ProductValue::get(&value, index).map_err(Self::Error::from)?;
                    self.write(SSAValue::from(*result), element)?;
                }

                Ok(())
            }
        }
    }
}
