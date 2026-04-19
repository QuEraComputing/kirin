use kirin_ir::{Block, CompileStage, StageMeta};

/// Dispatch table for multi-stage concrete interpretation.
///
/// Implementors provide a cursor type and a method to create a cursor for a
/// given stage. The cursor type is opaque here; the concrete multi-stage
/// interpreter composes stage-specific `ConcreteInterp` instances.
pub trait StageDispatch<S: StageMeta, V>: Sized {
    type Cursor;

    fn make_cursor(&self, stage: CompileStage, entry: Block, args: Vec<V>) -> Option<Self::Cursor>;
}

/// Multi-stage concrete interpreter dispatch key.
pub struct MultiConcreteInterp<S: StageMeta, V, K: StageDispatch<S, V>> {
    _phantom: std::marker::PhantomData<(S, V, K)>,
}

impl<S: StageMeta, V, K: StageDispatch<S, V>> MultiConcreteInterp<S, V, K> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S: StageMeta, V, K: StageDispatch<S, V>> Default for MultiConcreteInterp<S, V, K> {
    fn default() -> Self {
        Self::new()
    }
}
