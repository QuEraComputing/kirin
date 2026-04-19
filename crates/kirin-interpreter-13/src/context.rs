use kirin_ir::{CompileStage, ResultValue, SpecializedFunction};

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct AbstractFrame {
    pub func: SpecializedFunction,
    pub stage: CompileStage,
    pub results: Vec<ResultValue>,
}
