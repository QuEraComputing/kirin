use kirin_ir::{CompileStage, Product, UnGraph};

use crate::{Body, EnvIndex, InterpreterError};

use super::{BlockFrame, CFGFrame, CallFrame, DiGraphFrame};

/// Completion payloads produced by the standard concrete frames.
///
/// Representation walkers report *what happened*; the parent frame decides
/// what it means. The protocol distinguishes the three relevant exits:
///
/// - [`Returned`](Completion::Returned): an explicit function `Return` was
///   executed. Every frame between the walker and the call boundary relays it
///   unchanged (dialect frames included), so it bubbles to the nearest
///   [`CallFrame`] — the only frame that frees the callee activation and
///   writes the caller's result slots.
/// - [`Yielded`](Completion::Yielded): a structured `Yield` terminated a
///   block body. The structured-operation frame that pushed the block (e.g.
///   `scf.if`/`scf.for`) consumes the carried values. A [`CallFrame`]
///   rejects it: a callable Block or CFG must exit with `Return`, never a
///   structured yield.
/// - [`Finished`](Completion::Finished): the body ran to its natural end
///   with these values — a digraph's declared output yields, or a dialect
///   frame's finished sub-computation. A frame that pushed the child writes
///   them into the push's result slots; a [`CallFrame`] accepts them as the
///   call's returned values (a callable DiGraph's outputs are its returns).
pub enum Completion<V> {
    /// An explicit function `Return` with these values; bubbles to the
    /// enclosing [`CallFrame`].
    Returned(Product<V>),
    /// A structured `Yield` with these carried values; consumed by the
    /// dialect frame that pushed the block body.
    Yielded(Product<V>),
    /// Natural completion of a body/sub-computation with these values;
    /// delivered to whoever entered it (pusher or [`CallFrame`]).
    Finished(Product<V>),
}

/// The entry context handed to a dialect/compiler-supplied callable-UnGraph
/// policy: the callee stage, the callee activation (owned by the awaiting
/// [`CallFrame`], never by the policy frame), the graph, and the entry
/// arguments for its boundary ports.
pub struct UnGraphEntry<V> {
    pub stage: CompileStage,
    pub index: EnvIndex,
    pub graph: UnGraph,
    pub args: Product<V>,
}

/// Construction trait letting any total frame enum embed the standard
/// concrete frames.
///
/// The default [`StandardFrame`](super::StandardFrame) implements it
/// trivially; a language that adds structured-control dialects implements it
/// on its own enum to reuse the representation walkers and [`CallFrame`]
/// while adding its own dialect frames.
///
/// [`Body`](crate::Body) is a deliberately closed enum, so [`CallFrame`]
/// matches it exhaustively and maps each representation to its default
/// walker — except `UnGraph`, whose traversal is a policy this trait's
/// [`from_ungraph_entry`](Self::from_ungraph_entry) hook supplies.
pub trait FrameBuild<V, E>: Sized {
    fn from_block(frame: BlockFrame<V, E>) -> Self;
    fn from_cfg(frame: CFGFrame<V, E>) -> Self;
    fn from_call(frame: CallFrame<V>) -> Self;
    fn from_digraph(frame: DiGraphFrame<V, E>) -> Self;

    /// Build the entry frame for a **callable** `UnGraph` body.
    ///
    /// There is no framework default: an undirected graph has no inherent
    /// producer/consumer direction, control-flow successor, or topological
    /// execution order — its semantics (graph rewriting, circuits, constraint
    /// propagation, …) belong to the dialect/compiler. A language with such
    /// semantics overrides this to construct its own policy frame; everyone
    /// else inherits this rejection, so total frame enums carry no meaningless
    /// UnGraph boilerplate. (Nested, uncallable UnGraph operations don't come
    /// through here — a dialect frame enters them via
    /// [`SparseForwardEffect::Push`](crate::SparseForwardEffect::Push).)
    fn from_ungraph_entry(entry: UnGraphEntry<V>) -> Result<Self, E>
    where
        E: From<InterpreterError>,
    {
        Err(E::from(InterpreterError::NoDefaultWalker(Body::UnGraph(
            entry.graph,
        ))))
    }
}
