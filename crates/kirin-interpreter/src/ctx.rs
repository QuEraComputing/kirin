use kirin_ir::{CompileStage, Product, SSAValue, Statement};

use crate::{EnvIndex, InterpreterError};

/// The interpreter context seen by dialect code.
///
/// Implemented by engines ([`ConcreteInterpreter`](crate::ConcreteInterpreter),
/// [`AbstractInterpreter`](crate::AbstractInterpreter)), never by dialect or
/// compiler authors. Dialect impls are generic over `I: Interp` and constrain
/// `I::Value` with plain value-domain bounds (`Add`, `BranchCondition`, ...)
/// and `I::Error` with `From<TheirError>`.
pub trait Interp: Sized {
    /// The value domain: concrete values for execution, lattice elements for
    /// abstract interpretation.
    type Value: Clone;
    /// The total error type of the interpreter run.
    type Error: From<InterpreterError>;

    /// Read an SSA value from an activation. Prefer [`Ctx::read`].
    fn env_read(&self, env: EnvIndex, value: SSAValue) -> Result<Self::Value, Self::Error>;
    /// Write an SSA value into an activation. Prefer [`Ctx::write`].
    fn env_write(
        &mut self,
        env: EnvIndex,
        value: SSAValue,
        data: Self::Value,
    ) -> Result<(), Self::Error>;
}

/// Per-statement execution context handed to [`Interpretable::interpret`]
/// (crate::Interpretable::interpret).
///
/// Bundles the interpreter with the current stage, statement, and SSA
/// activation so dialect code reads and writes values without tracking
/// environment indices or locations.
pub struct Ctx<'a, I> {
    interp: &'a mut I,
    stage: CompileStage,
    statement: Statement,
    env: EnvIndex,
}

impl<'a, I: Interp> Ctx<'a, I> {
    pub fn new(
        interp: &'a mut I,
        stage: CompileStage,
        statement: Statement,
        env: EnvIndex,
    ) -> Self {
        Self {
            interp,
            stage,
            statement,
            env,
        }
    }

    /// The stage the current statement belongs to.
    pub fn stage(&self) -> CompileStage {
        self.stage
    }

    /// The statement being interpreted.
    pub fn statement(&self) -> Statement {
        self.statement
    }

    /// The current SSA activation.
    pub fn env(&self) -> EnvIndex {
        self.env
    }

    /// Escape hatch: access the interpreter directly.
    pub fn interp(&mut self) -> &mut I {
        self.interp
    }

    /// Read one SSA value.
    pub fn read(&self, value: impl Into<SSAValue>) -> Result<I::Value, I::Error> {
        self.interp.env_read(self.env, value.into())
    }

    /// Read a list of SSA values into a [`Product`].
    pub fn read_many(&self, values: &[SSAValue]) -> Result<Product<I::Value>, I::Error> {
        values.iter().map(|value| self.read(*value)).collect()
    }

    /// Write one SSA value.
    pub fn write(&mut self, value: impl Into<SSAValue>, data: I::Value) -> Result<(), I::Error> {
        self.interp.env_write(self.env, value.into(), data)
    }

    /// Destructure a [`Product`] into result slots, checking arity.
    pub fn write_results<T: Into<SSAValue> + Copy>(
        &mut self,
        values: &[T],
        data: Product<I::Value>,
    ) -> Result<(), I::Error> {
        if values.len() != data.len() {
            return Err(I::Error::from(InterpreterError::ProductArityMismatch {
                expected: values.len(),
                actual: data.len(),
            }));
        }
        for (value, data) in values.iter().zip(data) {
            self.write(*value, data)?;
        }
        Ok(())
    }
}

/// Narrow SSA environment view handed to [`ScopeHook`](crate::ScopeHook)
/// implementations: read/write access to the scope's activation, nothing else.
pub trait EnvOps<V, E> {
    fn read(&self, value: SSAValue) -> Result<V, E>;
    fn write(&mut self, value: SSAValue, data: V) -> Result<(), E>;
}

impl<I: Interp> EnvOps<I::Value, I::Error> for Ctx<'_, I> {
    fn read(&self, value: SSAValue) -> Result<I::Value, I::Error> {
        Ctx::read(self, value)
    }

    fn write(&mut self, value: SSAValue, data: I::Value) -> Result<(), I::Error> {
        Ctx::write(self, value, data)
    }
}

/// Engine-internal [`EnvOps`] over a specific activation.
pub(crate) struct EngineEnv<'a, I> {
    pub(crate) interp: &'a mut I,
    pub(crate) env: EnvIndex,
}

impl<I: Interp> EnvOps<I::Value, I::Error> for EngineEnv<'_, I> {
    fn read(&self, value: SSAValue) -> Result<I::Value, I::Error> {
        self.interp.env_read(self.env, value)
    }

    fn write(&mut self, value: SSAValue, data: I::Value) -> Result<(), I::Error> {
        self.interp.env_write(self.env, value, data)
    }
}
