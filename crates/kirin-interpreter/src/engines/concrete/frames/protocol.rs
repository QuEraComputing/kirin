use kirin_ir::{Block, CFG, CompileStage, DiGraph, Product, UnGraph};

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

/// The entry context for **one** callable body representation `B`, handed to a
/// [`CallBodyFramePolicy`].
///
/// Generalizes [`UnGraphEntry`], which is the `B = UnGraph` case retained for
/// the existing escape hatch. Note what is *not* here: the callee activation is
/// only *borrowed* — `index` names the activation the awaiting [`CallFrame`]
/// allocated and will free exactly once. A policy builds a walker over it; it
/// never owns its lifetime.
pub struct BodyFrameEntry<B, V> {
    pub stage: CompileStage,
    pub index: EnvIndex,
    pub body: B,
    pub args: Product<V>,
}

impl<V> From<BodyFrameEntry<UnGraph, V>> for UnGraphEntry<V> {
    fn from(entry: BodyFrameEntry<UnGraph, V>) -> Self {
        UnGraphEntry {
            stage: entry.stage,
            index: entry.index,
            graph: entry.body,
            args: entry.args,
        }
    }
}

/// Which walker enters a **callable** body of each representation.
///
/// This is the *body-entry* half of [`CallFrame`], split out so the two
/// concerns are separately replaceable:
///
/// - the **call convention** — resolve the callee, allocate its activation,
///   ask [`FunctionEntry`](crate::FunctionEntry) for the body, suspend,
///   validate the completion kind, free the activation exactly once, bind the
///   results — stays in [`CallFrame`] and is *not* configurable. It is where
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
/// Selected by the compiler/language author through the concrete total frame
/// type's [`FrameBuild::BodyFrames`]. A dialect crate may *offer* reusable
/// walkers or policies, but a callable dialect should not permanently fix one
/// traversal for every engine.
pub trait CallBodyFramePolicy<V, E, F> {
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
/// | `UnGraph` | [`FrameBuild::from_ungraph_entry`] — `NoDefaultWalker` unless overridden |
///
/// `CallFrame<V>` means `CallFrame<V, DefaultBodyFrames>`, so nothing changes
/// for a language that does not opt in.
pub struct DefaultBodyFrames;

impl<V, E, F> CallBodyFramePolicy<V, E, F> for DefaultBodyFrames
where
    V: Clone,
    E: From<InterpreterError>,
    F: FrameBuild<V, E>,
{
    fn from_cfg(entry: BodyFrameEntry<CFG, V>) -> Result<F, E> {
        Ok(F::from_cfg(CFGFrame::new(
            entry.stage,
            entry.index,
            entry.body,
            entry.args,
        )))
    }

    fn from_block(entry: BodyFrameEntry<Block, V>) -> Result<F, E> {
        Ok(F::from_block(BlockFrame::new(
            entry.stage,
            entry.index,
            entry.body,
            entry.args,
        )))
    }

    fn from_digraph(entry: BodyFrameEntry<DiGraph, V>) -> Result<F, E> {
        Ok(F::from_digraph(DiGraphFrame::new(
            entry.stage,
            entry.index,
            entry.body,
            entry.args,
        )))
    }

    /// Delegates to the pre-existing escape hatch, so a language that already
    /// supplies a callable-UnGraph walker keeps working untouched.
    fn from_ungraph(entry: BodyFrameEntry<UnGraph, V>) -> Result<F, E> {
        F::from_ungraph_entry(entry.into())
    }
}

/// Construction trait letting any total frame enum embed the standard
/// concrete frames.
///
/// The default [`StandardFrame`](super::StandardFrame) implements it
/// trivially; a language that adds structured-control dialects implements it
/// on its own enum to reuse the representation walkers and [`CallFrame`]
/// while adding its own dialect frames.
///
/// Two traits sit next to each other here; they answer different questions:
///
/// - **`FrameBuild`** — *injection*: how is an already-built frame wrapped into
///   the total frame type `F`? One constructor per framework frame, so a member
///   frame can re-wrap itself without knowing what `F` is.
/// - **[`CallBodyFramePolicy`]** — *selection*: which walker enters a **callable**
///   body of a given representation? Chosen per language via
///   [`BodyFrames`](Self::BodyFrames).
///
/// [`Body`](crate::Body) is a deliberately closed enum, so [`CallFrame`]
/// matches it exhaustively; the policy decides only *which frame* each arm
/// builds. `UnGraph` keeps its dedicated
/// [`from_ungraph_entry`](Self::from_ungraph_entry) hook, which
/// [`DefaultBodyFrames`] delegates to, so languages that already supply a
/// callable-UnGraph walker are unaffected.
pub trait FrameBuild<V, E>: Sized {
    /// Which walkers this frame type uses to enter a **callable** body.
    ///
    /// Defaults to [`DefaultBodyFrames`] for every enum that does not opt in;
    /// see [`CallBodyFramePolicy`]. Deliberately *unbounded* here: bounding it
    /// as `CallBodyFramePolicy<V, E, Self>` makes checking
    /// `type BodyFrames = DefaultBodyFrames` require `Self: FrameBuild<V, E>`,
    /// i.e. the very impl being checked. The obligation is instead attached
    /// where the policy is *used*, in `CallFrame`'s [`Frame`](crate::Frame)
    /// impl.
    type BodyFrames;

    fn from_block(frame: BlockFrame<V, E>) -> Self;
    fn from_cfg(frame: CFGFrame<V, E>) -> Self;
    fn from_call(frame: CallFrame<V, Self::BodyFrames>) -> Self;
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
