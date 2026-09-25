use kirin_ir::{Block, CFG, CompileStage, DiGraph, Product, UnGraph};

use crate::{Body, EnvIndex, InterpreterError};

use super::{BlockFrame, CFGFrame, DiGraphFrame};

/// Completion payloads produced by the standard concrete frames.
///
/// Representation walkers report *what happened*; the parent frame decides
/// what it means. The protocol distinguishes the three relevant exits:
///
/// - [`Returned`](Completion::Returned): an explicit function `Return` was
///   executed. Every frame between the walker and the call boundary relays it
///   unchanged (dialect frames included), so it bubbles to the nearest
///   [`CallFrame`](crate::CallFrame) — the only frame that frees the callee activation and
///   writes the caller's result slots.
/// - [`Yielded`](Completion::Yielded): a structured `Yield` terminated a
///   block body. The structured-operation frame that pushed the block (e.g.
///   `scf.if`/`scf.for`) consumes the carried values. A [`CallFrame`](crate::CallFrame)
///   rejects it: a callable Block or CFG must exit with `Return`, never a
///   structured yield.
/// - [`Finished`](Completion::Finished): the body ran to its natural end
///   with these values — a digraph's declared output yields, or a dialect
///   frame's finished sub-computation. A frame that pushed the child writes
///   them into the push's result slots; a [`CallFrame`](crate::CallFrame) accepts them as the
///   call's returned values (a callable DiGraph's outputs are its returns).
pub enum Completion<V> {
    /// An explicit function `Return` with these values; bubbles to the
    /// enclosing [`CallFrame`](crate::CallFrame).
    Returned(Product<V>),
    /// A structured `Yield` with these carried values; consumed by the
    /// dialect frame that pushed the block body.
    Yielded(Product<V>),
    /// Natural completion of a body/sub-computation with these values;
    /// delivered to whoever entered it (pusher or [`CallFrame`](crate::CallFrame)).
    Finished(Product<V>),
}

/// The entry context for **one** callable body representation `B`, handed to a
/// [`CallBodyTraversal`].
///
/// The callee activation is only *borrowed* — `index` names the activation the awaiting [`CallFrame`](crate::CallFrame)
/// allocated and will free exactly once. A traversal builds a walker over it; it
/// never owns its lifetime.
pub struct BodyFrameEntry<B, V> {
    pub stage: CompileStage,
    pub index: EnvIndex,
    pub body: B,
    pub args: Product<V>,
}

/// Which walker enters a **callable** body of each representation.
///
/// This is the *body-entry* half of [`CallFrame`](crate::CallFrame), split out so the two
/// concerns are separately replaceable:
///
/// - the **call convention** — resolve the callee, allocate its activation,
///   ask [`FunctionEntry`](crate::FunctionEntry) for the body, suspend,
///   validate the completion kind, free the activation exactly once, bind the
///   results — stays in [`CallFrame`](crate::CallFrame) and is *not* configurable. It is where
///   double-frees would live.
/// - the **walker choice** — which frame traverses that body — is this trait.
///
/// So a language can say "walk my CFGs with my own scheduler" without forking
/// the lifecycle. [`Body`](crate::Body) stays a closed vocabulary; only the
/// frame chosen per variant becomes configurable.
///
/// **Concrete execution only.** Forward abstract interpretation does not
/// descend into a callee — `AbstractCallFrame` *summarizes* the call and the
/// fixpoint engine separately maps a callable body to an
/// [`Owner`](crate::Owner). Customizing that would be an abstract
/// body-entry/owner policy, not this one. The backward engines don't walk
/// callable bodies through a call frame at all.
///
/// The traversal returns the configuration's child representation `F`. Concrete
/// configurations currently use their private frame-stack-item type directly;
/// stack-item conversions remain localized at the composition root.
pub trait CallBodyTraversal<V, E, F> {
    fn from_cfg(entry: BodyFrameEntry<CFG, V>) -> Result<F, E>;
    fn from_block(entry: BodyFrameEntry<Block, V>) -> Result<F, E>;
    fn from_digraph(entry: BodyFrameEntry<DiGraph, V>) -> Result<F, E>;
    fn from_ungraph(entry: BodyFrameEntry<UnGraph, V>) -> Result<F, E>;
}

/// The framework's default callable-body walkers — today's exact behaviour:
///
/// | body | walker |
/// |---|---|
/// | `CFG` | [`CFGFrame`] |
/// | `Block` | [`BlockFrame`] |
/// | `DiGraph` | [`DiGraphFrame`] |
/// | `UnGraph` | `NoDefaultWalker` — a custom traversal must choose its semantics |
///
/// `CallFrame<V>` means `CallFrame<V, DefaultCallBodyTraversal>`, so nothing changes
/// for a language that does not opt in.
pub struct DefaultCallBodyTraversal;

impl<V, E, F> CallBodyTraversal<V, E, F> for DefaultCallBodyTraversal
where
    V: Clone,
    E: From<InterpreterError>,
    F: From<CFGFrame<V, E>> + From<BlockFrame<V, E>> + From<DiGraphFrame<V, E>>,
{
    fn from_cfg(entry: BodyFrameEntry<CFG, V>) -> Result<F, E> {
        Ok(CFGFrame::new(entry.stage, entry.index, entry.body, entry.args).into())
    }

    fn from_block(entry: BodyFrameEntry<Block, V>) -> Result<F, E> {
        Ok(BlockFrame::new(entry.stage, entry.index, entry.body, entry.args).into())
    }

    fn from_digraph(entry: BodyFrameEntry<DiGraph, V>) -> Result<F, E> {
        Ok(DiGraphFrame::new(entry.stage, entry.index, entry.body, entry.args).into())
    }

    fn from_ungraph(entry: BodyFrameEntry<UnGraph, V>) -> Result<F, E> {
        Err(E::from(InterpreterError::NoDefaultWalker(Body::UnGraph(
            entry.body,
        ))))
    }
}
