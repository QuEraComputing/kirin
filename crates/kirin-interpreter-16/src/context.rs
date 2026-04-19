use kirin_ir::{CompileStage, ResultValue, SpecializedFunction};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbstractFrame {
    pub func: SpecializedFunction,
    pub stage: CompileStage,
    pub results: Vec<ResultValue>,
}
