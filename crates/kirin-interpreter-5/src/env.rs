use kirin_interpreter::ProductValue;
use kirin_ir::{ResultValue, SSAValue};

use crate::error::InterpreterError;

/// The core interface for interpreter-5 dialect execution.
///
/// Dialect authors implement [`Interpretable<E>`] for their dialect types.
/// Named `Env` (execution environment) to avoid collision with "abstract domain"
/// from abstract interpretation (Cousot & Cousot), which is the meaning used in
/// `kirin-interval` and `kirin-interpreter`.
pub trait Env {
    type Value: Clone;
    type Effect;
    type Error: From<InterpreterError>;

    /// Return the canonical "advance" effect (no-op, move to next statement).
    fn advance() -> Self::Effect;

    /// Return the active compilation stage.
    fn current_stage(&self) -> kirin_ir::CompileStage;

    /// Read an SSA value from the current frame.
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;

    /// Write a result slot in the current frame.
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;

    /// Write an arbitrary SSA value (e.g. block argument) in the current frame.
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;

    /// Read multiple SSA values at once.
    fn read_many(&self, values: &[SSAValue]) -> Result<Vec<Self::Value>, Self::Error> {
        values.iter().map(|&v| self.read(v)).collect()
    }

    /// Write multiple result slots from a product value.
    fn write_product(
        &mut self,
        results: &[ResultValue],
        product: Self::Value,
    ) -> Result<(), Self::Error>
    where
        Self::Value: ProductValue,
    {
        if results.is_empty() {
            return Ok(());
        }
        if results.len() == 1 {
            self.write(results[0], product)?;
        } else if let Some(components) = product.as_product() {
            for (result, value) in results.iter().zip(components.iter()) {
                self.write(*result, value.clone())?;
            }
        } else {
            self.write(results[0], product)?;
        }
        Ok(())
    }
}

/// Dialect author trait: produce an effect from an env reference.
///
/// Implemented by dialect enum types to drive the interpreter. The env
/// provides read/write access and stage info.
pub trait Interpretable<E: Env> {
    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error>;
}
