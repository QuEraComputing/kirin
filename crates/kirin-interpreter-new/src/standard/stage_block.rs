use kirin_ir::{Block, CompileStage, Product};

use crate::EnvIndex;

/// Build a block frame for a block at a runtime-known compile stage.
///
/// Some interpreters work across multiple stages whose IRs use different
/// dialects. The concrete dialect is only known at runtime through
/// [`CompileStage`]. `StageBlockDispatch` lets such interpreters dispatch to
/// the correct dialect-typed [`BlockFrame`](crate::BlockFrame) and lift it
/// into the user's frame enum.
///
/// The user implements this once per interpreter shell, typically as a match
/// on the pipeline's stage enum.
pub trait StageBlockDispatch<F, E, V> {
    fn dispatch_stage_block(
        &mut self,
        stage: CompileStage,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E>;
}
