use kirin_ir::{CompileStage, Pipeline, SpecializedFunction, StageMeta};

use crate::error::InterpreterError;

pub trait CallDispatch<V, C>: StageMeta + Sized {
    fn make_call_cursor(
        pipeline: &Pipeline<Self>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<C, InterpreterError>;
}
