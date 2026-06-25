use kirin_ir::{CompileStage, Product, SSAValue, Statement};

use crate::{EnvIndex, ForwardEffect, InterpreterError};

/// Shared engine contract for concrete execution and analyses.
///
/// `Interp` names the value domain, error type, statement effect, and context
/// type used by an engine. SSA storage access lives on [`Env`], and traversal
/// lives in frame types.
pub trait Interp: Sized {
    /// The value domain: concrete values for execution, lattice elements for
    /// abstract interpretation.
    type Value: Clone;
    /// The total error type of the interpreter run.
    type Error: From<InterpreterError>;
    /// The per-statement effect/result produced by this analysis.
    type Effect;

    /// The context API handed to dialect rules.
    type Context<'a>: InterpretCtx<Value = Self::Value, Error = Self::Error, Effect = Self::Effect>
    where
        Self: 'a;

    /// Build the per-statement context dispatch hands to a dialect rule.
    fn context<'a>(
        &'a mut self,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Self::Context<'a>;
}

/// Marker trait for lattice-valued abstract interpretation engines.
///
/// This intentionally does not require forward env access, widening, or a
/// universal join API; those belong to concrete engine specializations. The
/// value domain only needs to be `Clone`; lattice operations like
/// [`HasBottom`](kirin_ir::HasBottom)/[`HasTop`](kirin_ir::HasTop) are required
/// by concrete engine specializations, not by this marker.
pub trait AbstractInterpreter: Interp {}

/// SSA storage access used by [`ValueContext`].
pub trait Env: Interp {
    /// Read an SSA value from an activation.
    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<Self::Value, Self::Error>;
    /// Write an SSA value into an activation.
    fn env_write(
        &mut self,
        index: EnvIndex,
        value: SSAValue,
        data: Self::Value,
    ) -> Result<(), Self::Error>;
}

/// Forward interpretation flavor: env access plus [`ForwardEffect`].
///
/// The associated frame type is exposed only because [`ForwardEffect::Push`]
/// carries a frame. Ordinary dialects do not name it.
pub trait ForwardInterp:
    Env + Interp<Effect = ForwardEffect<<Self as Interp>::Value, Self::Frame>>
{
    /// The engine's total frame type, carried by [`ForwardEffect::Push`].
    type Frame;
}

impl<V, F, I> ForwardInterp for I
where
    I: Env + Interp<Value = V, Effect = ForwardEffect<V, F>>,
{
    type Frame = F;
}

/// Minimal contract every interpretation context exposes to a dialect rule.
pub trait InterpretCtx {
    /// The value domain: concrete values for execution, lattice elements for
    /// abstract interpretation, live-sets for liveness, ...
    type Value: Clone;
    /// The total error type of the interpreter run.
    type Error: From<InterpreterError>;
    /// The per-statement effect/result this context's rules produce.
    type Effect;
}

/// Per-statement value context.
///
/// Dialect rules use this to read and write SSA values without handling
/// activation indices directly.
pub struct ValueContext<'a, I> {
    interp: &'a mut I,
    stage: CompileStage,
    statement: Statement,
    index: EnvIndex,
}

impl<'a, I: Interp> ValueContext<'a, I> {
    pub fn new(
        interp: &'a mut I,
        stage: CompileStage,
        statement: Statement,
        index: EnvIndex,
    ) -> Self {
        Self {
            interp,
            stage,
            statement,
            index,
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
    pub fn index(&self) -> EnvIndex {
        self.index
    }

    /// Escape hatch: access the interpreter directly. Used by structured
    /// dialects that build an engine-specific frame to push (e.g. `kirin-scf`).
    pub fn interp(&mut self) -> &mut I {
        self.interp
    }
}

impl<'a, I: Interp> InterpretCtx for ValueContext<'a, I> {
    type Value = I::Value;
    type Error = I::Error;
    type Effect = I::Effect;
}

/// SSA value read/write helpers used by dialect rules.
impl<'a, I: Env> ValueContext<'a, I> {
    /// Read one SSA value.
    pub fn read(&self, value: impl Into<SSAValue>) -> Result<I::Value, I::Error> {
        self.interp.env_read(self.index, value.into())
    }

    /// Read a list of SSA values into a [`Product`].
    pub fn read_many(&self, values: &[SSAValue]) -> Result<Product<I::Value>, I::Error> {
        values.iter().map(|value| self.read(*value)).collect()
    }

    /// Write one SSA value.
    pub fn write(&mut self, value: impl Into<SSAValue>, data: I::Value) -> Result<(), I::Error> {
        self.interp.env_write(self.index, value.into(), data)
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
