//! Shared backbone-test fixtures.

use kirin_ir::{CompileStage, Statement};

use crate::{EnvIndex, Interp, InterpreterError};

/// A minimal [`Interp`] used as the wrapped `inner` for backbone tests.
///
/// The backbone tests exercise only the owner/summary/dependency machinery;
/// their frames complete synthetically without dispatching a dialect rule, so
/// the location accessors are unreachable (mirrors the liveness interps, whose
/// structural analyses never dispatch through `Interpretable`).
pub(super) struct UnitInterp;

impl Interp for UnitInterp {
    type Value = ();
    type Error = InterpreterError;
    type Effect = ();
    type Kind = ();

    fn stage(&self) -> CompileStage {
        unimplemented!("backbone test interp has no IR location")
    }

    fn statement(&self) -> Statement {
        unimplemented!("backbone test interp has no IR location")
    }

    fn index(&self) -> EnvIndex {
        unimplemented!("backbone test interp has no IR location")
    }
}
