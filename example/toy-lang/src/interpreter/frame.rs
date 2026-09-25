//! Total frame types for the toy language.
//!
//! The toy language uses `kirin-scf`, whose `scf.for` pushes a dialect-owned
//! loop frame ([`ScfForFrame`]/[`AbstractScfForFrame`]). A language that uses
//! such a dialect explicitly composes the standard framework frames — the representation walkers
//! ([`BlockFrame`]/[`CFGFrame`]/[`DiGraphFrame`]) and the [`CallFrame`] call
//! boundary — plus the dialect frames. Each total stack-item enum is a
//! composition root; its member frames do not know which enum stores them.

use kirin_interpreter::engine::{
    AbstractBlockFrame, AbstractCallFrame, AbstractDiGraphFrame, BlockFrame, CFGFrame, CallFrame,
    CallRequest, DiGraphFrame, Frame,
};
use kirin_scf::{AbstractScfForFrame, AbstractScfIfFrame, ScfForFrame, ScfIfFrame};

// ===========================================================================
// Concrete
// ===========================================================================

/// The toy language's private concrete continuation stack element.
///
/// This enum is the language's explicit composition root: it records which
/// framework and dialect-owned computations may coexist on one stack. Member
/// frames never name it or construct its variants.
#[derive(Frame)]
pub(crate) enum ToyFrame<V, E> {
    Block(BlockFrame<V, E>),
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
    ScfIf(ScfIfFrame<V, E>),
    ScfFor(ScfForFrame<V, E>),
}

impl<V, E> From<CallRequest<V>> for ToyFrame<V, E> {
    fn from(request: CallRequest<V>) -> Self {
        Self::Call(request.into())
    }
}

// ===========================================================================
// Abstract
// ===========================================================================

/// Toy's abstract stack-item composition: framework traversal plus SCF.
///
/// The graph variant is present because the generic sparse-forward engine can
/// select a graph body even though the current toy programs use CFG bodies.
#[derive(Frame)]
pub enum ToyAbstractFrame<V, E, K> {
    Block(AbstractBlockFrame<V, E, K>),
    Call(AbstractCallFrame<V, E, K>),
    DiGraph(AbstractDiGraphFrame<V, E, K>),
    ScfIf(AbstractScfIfFrame<V, E, K>),
    ScfFor(AbstractScfForFrame<V, E, K>),
}

// ===========================================================================
// Dense backward (classic per-point liveness)
// ===========================================================================

use kirin_interpreter::engine::DenseBlockFrame;
use kirin_scf::{DenseScfForFrame, DenseScfIfFrame};

/// Toy's private dense-backward stack item: the reverse block walk plus the
/// SCF dense continuations (arm join and loop-carried fixpoint).
#[derive(Frame)]
pub(crate) enum ToyDenseBackwardFrame<V, E> {
    Block(DenseBlockFrame<V, E>),
    ScfIf(DenseScfIfFrame<V, E>),
    ScfFor(DenseScfForFrame<V, E>),
}
