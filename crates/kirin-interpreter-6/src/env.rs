use kirin_interpreter::ProductValue;
use kirin_ir::{CompileStage, ResultValue, SSAValue};

use crate::error::InterpreterError;
use crate::lift::Lift;

/// Core execution environment interface.
///
/// Dialect authors implement [`Interpretable<E>`] for their op types, parameterized
/// over any `E: Env`. Named `Env` (execution environment) to avoid collision with
/// "abstract domain" from abstract interpretation (Cousot & Cousot).
pub trait Env {
    type Value: Clone;
    type Effect;
    type Error: From<InterpreterError>;

    fn current_stage(&self) -> CompileStage;

    fn read(&self, ssa: SSAValue) -> Result<Self::Value, Self::Error>;
    fn write(&mut self, r: ResultValue, v: Self::Value) -> Result<(), Self::Error>;
    fn write_ssa(&mut self, ssa: SSAValue, v: Self::Value) -> Result<(), Self::Error>;

    fn read_many(&self, values: &[SSAValue]) -> Result<Vec<Self::Value>, Self::Error> {
        values.iter().map(|&v| self.read(v)).collect()
    }

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

    /// Produce the "advance to next statement" effect.
    ///
    /// Available when `Self::Effect` can represent an advance, i.e. when it
    /// implements `Lift<()>` (which `Core<V, C>` satisfies via `Core::Advance`).
    fn advance() -> Self::Effect
    where
        Self::Effect: Lift<()>,
    {
        Self::Effect::lift(())
    }
}

/// Dialect author trait: produce an effect from a statement in environment `E`.
///
/// # Two usage levels
///
/// **Op level** — the concrete op returns its dialect-specific effect:
/// ```rust,ignore
/// impl<E: Env> Interpretable<E> for IfOp<T> {
///     type DialectEffect = Core<E::Value, E::Cursor>; // requires E: ConcreteDomain
///     fn interpret(&self, env: &mut E) -> Result<Self::DialectEffect, E::Error> { ... }
/// }
/// ```
///
/// **Dialect wrapper level** — the enum wrapper lifts dialect effects to `E::Effect`:
/// ```rust,ignore
/// impl<E: Env> Interpretable<E> for SCF<T>
/// where
///     E::Effect: Lift<Core<E::Value, E::Cursor>>,
/// {
///     type DialectEffect = E::Effect;
///     fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
///         match self {
///             Self::If(op) => op.interpret(env).map(|e| E::Effect::lift(e)),
///             ...
///         }
///     }
/// }
/// ```
pub trait Interpretable<E: Env> {
    /// The effect produced by this op or dialect.
    ///
    /// At the op level: a dialect-specific type (e.g. `Core<V, C>`, `QuantumEffect<V>`).
    /// At the dialect wrapper level: `E::Effect` (already lifted to language-level).
    type DialectEffect;
    fn interpret(&self, env: &mut E) -> Result<Self::DialectEffect, E::Error>;
}
