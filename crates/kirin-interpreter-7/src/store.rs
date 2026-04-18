use kirin_interpreter::ProductValue;
use kirin_ir::{ResultValue, SSAValue};

use crate::error::InterpreterError;

/// SSA value read/write interface.
///
/// The base storage layer shared by both `ConcreteInterp` and `AbstractInterp`.
/// Separated from `Interp` so that dialect ops can be generic over the minimal
/// interface they actually use.
pub trait Store {
    type Value: Clone;
    type Error: From<InterpreterError>;

    fn read(&self, ssa: SSAValue) -> Result<Self::Value, Self::Error>;
    fn write_result(&mut self, r: ResultValue, v: Self::Value) -> Result<(), Self::Error>;
    fn write_ssa(&mut self, ssa: SSAValue, v: Self::Value) -> Result<(), Self::Error>;

    fn read_many(&self, ssas: &[SSAValue]) -> Result<Vec<Self::Value>, Self::Error> {
        ssas.iter().map(|&ssa| self.read(ssa)).collect()
    }

    fn write_product(&mut self, results: &[ResultValue], v: Self::Value) -> Result<(), Self::Error>
    where
        Self::Value: ProductValue,
    {
        if results.is_empty() {
            return Ok(());
        }
        if results.len() == 1 {
            self.write_result(results[0], v)?;
        } else if let Some(components) = v.as_product() {
            for (result, value) in results.iter().zip(components.iter()) {
                self.write_result(*result, value.clone())?;
            }
        } else {
            self.write_result(results[0], v)?;
        }
        Ok(())
    }
}
