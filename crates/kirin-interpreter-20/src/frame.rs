use kirin_ir::{CompileStage, ResultValue, SSAValue, SpecializedFunction};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct Frame<V> {
    callee: SpecializedFunction,
    stage: CompileStage,
    values: FxHashMap<SSAValue, V>,
    caller_results: Vec<ResultValue>,
}

impl<V> Frame<V> {
    pub fn new(
        callee: SpecializedFunction,
        stage: CompileStage,
        caller_results: Vec<ResultValue>,
    ) -> Self {
        Self {
            callee,
            stage,
            values: FxHashMap::default(),
            caller_results,
        }
    }

    pub fn callee(&self) -> SpecializedFunction {
        self.callee
    }
    pub fn stage(&self) -> CompileStage {
        self.stage
    }
    pub fn caller_results(&self) -> &[ResultValue] {
        &self.caller_results
    }
    pub fn read(&self, value: SSAValue) -> Option<&V> {
        self.values.get(&value)
    }
    pub fn write(&mut self, result: ResultValue, value: V) -> Option<V> {
        self.values.insert(result.into(), value)
    }
    pub fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Option<V> {
        self.values.insert(ssa, value)
    }
}
