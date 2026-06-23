use kirin_ir::{CompileStage, Product, SSAValue, Statement};

use crate::{EnvIndex, ForwardEffect, InterpreterError};

/// The interpreter/analysis driver seen by dialect code.
///
/// Implemented by engines ([`ConcreteInterpreter`](crate::ConcreteInterpreter),
/// [`AbstractInterpreter`](crate::AbstractInterpreter)), never by dialect or
/// compiler authors. It exposes, as associated types, the value domain, the
/// total error, and the per-statement [`Effect`](Interp::Effect) algebra — the
/// last of which is **analysis-specific**, so different analyses do not share one
/// universal effect enum.
///
/// Dialect rules are generic over `I` and constrain `I::Value` with plain
/// value-domain bounds (`Add`, `BranchCondition`, ...) and `I::Error` with
/// `From<TheirError>`. Forward rules additionally bound `I: ForwardInterp`, which
/// pins `I::Effect` to [`ForwardEffect`].
///
/// Traversal types stay out of `Interp`: the frame type is the engine's own `F`
/// generic parameter (e.g. `ConcreteInterpreter<.., F>`), so frames remain
/// customizable without an unused associated type here.
pub trait Interp: Sized {
    /// The value domain: concrete values for execution, lattice elements for
    /// abstract interpretation.
    type Value: Clone;
    /// The total error type of the interpreter run.
    type Error: From<InterpreterError>;
    /// The per-statement effect algebra an [`Interpretable`](crate::Interpretable)
    /// rule produces for this analysis — the statement→frame message. It is
    /// **analysis-specific**, not a single universal enum: forward execution and
    /// abstract interpretation use [`ForwardEffect`]; a future backward analysis
    /// defines its own result/effect type (e.g. a live-set) without touching the
    /// framework or other analyses.
    type Effect;

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

/// The **forward** value-interpretation flavor of [`Interp`]: one whose
/// [`Effect`](Interp::Effect) is the forward control algebra [`ForwardEffect`].
///
/// Forward dialect rules bound `I: ForwardInterp` so they can build and return
/// `ForwardEffect` values where the trait expects `I::Effect` (the two are the
/// same type for a `ForwardInterp`). The associated [`Frame`](ForwardInterp::Frame)
/// is the engine's total frame type — exposed here so a structured dialect can
/// name it (e.g. to build a [`ForwardEffect::Push`]) without it leaking into
/// the [`Interp`] base trait. A blanket impl makes every such `Interp` a
/// `ForwardInterp` automatically; nobody implements it by hand. Backward
/// analyses define their own `Interp` flavor with a different `Effect`.
pub trait ForwardInterp:
    Interp<Effect = ForwardEffect<<Self as Interp>::Value, Self::Frame>>
{
    /// The engine's total frame type, carried by [`ForwardEffect::Push`].
    type Frame;
}

impl<V, F, I> ForwardInterp for I
where
    I: Interp<Value = V, Effect = ForwardEffect<V, F>>,
{
    type Frame = F;
}

/// Per-statement execution context handed to
/// [`Interpretable::interpret`](crate::Interpretable::interpret).
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
