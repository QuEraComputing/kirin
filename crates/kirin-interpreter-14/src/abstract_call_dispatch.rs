use kirin_ir::{Block, CompileStage, Pipeline, SpecializedFunction, StageMeta};

use crate::error::InterpreterError;

pub trait AbstractCallDispatch<V, C>: StageMeta + Sized {
    fn make_abstract_cursor(
        pipeline: &Pipeline<Self>,
        stage_id: CompileStage,
        block: Block,
        args: Vec<V>,
    ) -> C;

    fn entry_block_for(
        pipeline: &Pipeline<Self>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError>;
}
