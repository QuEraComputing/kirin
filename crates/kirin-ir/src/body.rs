use crate::{Block, CFG, DiGraph, UnGraph};

/// A handle to one of Kirin's computation representations.
///
/// The owning operation determines the body's meaning. This sum records its
/// representation without choosing traversal, binding arguments, or requiring
/// an interpreter. Handles are read in the stage that owns them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Body {
    Block(Block),
    CFG(CFG),
    DiGraph(DiGraph),
    UnGraph(UnGraph),
}

impl From<Block> for Body {
    fn from(block: Block) -> Self {
        Self::Block(block)
    }
}

impl From<CFG> for Body {
    fn from(cfg: CFG) -> Self {
        Self::CFG(cfg)
    }
}

impl From<DiGraph> for Body {
    fn from(graph: DiGraph) -> Self {
        Self::DiGraph(graph)
    }
}

impl From<UnGraph> for Body {
    fn from(graph: UnGraph) -> Self {
        Self::UnGraph(graph)
    }
}

/// Discover the Kirin body explicitly designated as a definition's call implementation.
///
/// `#[derive(Dialect)]` generates this capability: a direct definition marks
/// one `Block`, `CFG`, `DiGraph`, or `UnGraph` field with
/// `#[kirin(callable_body)]`; an unmarked definition returns `None`.
/// `#[wraps]` definitions delegate to their wrapped operation automatically.
/// Merely owning a body does not imply callability.
///
/// This capability is independent of [`crate::HasSignature`]: a lambda can
/// expose a body without a stored signature, and an external declaration can
/// carry a signature without a Kirin implementation. Discovery neither binds
/// forward arguments nor seeds a backward analysis.
pub trait HasCallableBody {
    fn callable_body(&self) -> Option<Body>;
}
