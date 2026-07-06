//! `Interp` (and friends) for the driver, by delegation to the wrapped `inner`.
//!
//! The driver keeps the interpreter model intact: `inner: I` remains the single
//! source of [`Value`](Interp::Value) / [`Error`](Interp::Error) /
//! [`Effect`](Interp::Effect) / [`Semantics`](Interp::Semantics) and the current
//! location.
//! Because the driver also delegates [`Env`] (when `I: Env`) and reports
//! `Effect = I::Effect`, a forward `inner` transparently gives the driver the
//! blanket [`SparseForwardInterp`](crate::SparseForwardInterp) impl and its
//! `read`/`write` helpers.

use kirin_ir::{CompileStage, SSAValue, Statement};

use crate::{Env, EnvIndex, Interp};

use super::{FixpointProfile, StandardFixpointInterpreter};

impl<I, P, Store, Deps> Interp for StandardFixpointInterpreter<I, P, Store, Deps>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    type Value = I::Value;
    type Error = I::Error;
    type Effect = I::Effect;
    type Semantics = I::Semantics;

    fn stage(&self) -> CompileStage {
        self.inner.stage()
    }

    fn statement(&self) -> Statement {
        self.inner.statement()
    }

    fn index(&self) -> EnvIndex {
        self.inner.index()
    }
}

impl<I, P, Store, Deps> Env for StandardFixpointInterpreter<I, P, Store, Deps>
where
    I: Env,
    P: FixpointProfile<I>,
{
    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<Self::Value, Self::Error> {
        self.inner.env_read(index, value)
    }

    fn env_write(
        &mut self,
        index: EnvIndex,
        value: SSAValue,
        data: Self::Value,
    ) -> Result<(), Self::Error> {
        self.inner.env_write(index, value, data)
    }
}
