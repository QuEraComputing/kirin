use kirin_ir::{Block, CompileStage, StageMeta};

/// Dispatch table for multi-stage abstract interpretation.
pub trait StageAnalyze<S: StageMeta, V>: Sized {
    type Cursor;

    fn make_cursor(&self, stage: CompileStage, entry: Block, args: Vec<V>) -> Option<Self::Cursor>;
}

/// Multi-stage abstract interpreter dispatch key.
pub struct MultiAbstractInterp<S: StageMeta, V, K: StageAnalyze<S, V>> {
    _phantom: std::marker::PhantomData<(S, V, K)>,
}

impl<S: StageMeta, V, K: StageAnalyze<S, V>> MultiAbstractInterp<S, V, K> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S: StageMeta, V, K: StageAnalyze<S, V>> Default for MultiAbstractInterp<S, V, K> {
    fn default() -> Self {
        Self::new()
    }
}
