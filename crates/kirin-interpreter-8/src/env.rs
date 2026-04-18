use std::convert::Infallible;

use kirin_interpreter::ProductValue;
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageInfo, StageMeta, Symbol,
};

use crate::error::InterpreterError;

/// Unified domain trait for the interpreter.
///
/// Merges the `Store` + `Interp` split from interpreter-7 into a single trait.
/// Both `ConcreteInterp` and `AbstractInterp` implement this.
pub trait Env {
    type Value: Clone;
    type Ext;
    type Error: From<InterpreterError>;
    type Stages: StageMeta;

    fn current_stage(&self) -> CompileStage;
    fn pipeline(&self) -> &Pipeline<Self::Stages>;

    fn read_value(&self, ssa: SSAValue) -> Result<Self::Value, Self::Error>;
    fn write_result(&mut self, r: ResultValue, v: Self::Value) -> Result<(), Self::Error>;
    fn write_ssa(&mut self, ssa: SSAValue, v: Self::Value) -> Result<(), Self::Error>;

    fn read_many(&self, ssas: &[SSAValue]) -> Result<Vec<Self::Value>, Self::Error> {
        ssas.iter().map(|&ssa| self.read_value(ssa)).collect()
    }

    fn stage_info_for<L: Dialect>(&self, id: CompileStage) -> Option<&StageInfo<L>>
    where
        Self::Stages: HasStageInfo<L>,
    {
        self.pipeline().stage(id)?.try_stage_info()
    }

    /// Resolve a function symbol to a `SpecializedFunction` in the given stage.
    fn resolve_function_for<L: Dialect>(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, Self::Error>
    where
        Self::Stages: HasStageInfo<L>,
        Self::Error: From<InterpreterError>,
    {
        let stage_container = self
            .pipeline()
            .stage(stage_id)
            .ok_or_else(|| Self::Error::from(InterpreterError::MissingEntry))?;
        let stage_info: &StageInfo<L> = stage_container
            .try_stage_info()
            .ok_or_else(|| Self::Error::from(InterpreterError::MissingEntry))?;
        let function = self
            .pipeline()
            .resolve_function(stage_info, target)
            .ok_or_else(|| Self::Error::from(InterpreterError::MissingEntry))?;
        let staged_function = self
            .pipeline()
            .function_info(function)
            .and_then(|info| info.staged_function(stage_id))
            .ok_or_else(|| Self::Error::from(InterpreterError::MissingEntry))?;
        staged_function
            .get_info(stage_info)
            .ok_or_else(|| Self::Error::from(InterpreterError::MissingEntry))?
            .unique_live_specialization()
            .map_err(|_| {
                Self::Error::from(InterpreterError::UnhandledEffect(
                    "ambiguous specialization".into(),
                ))
            })
    }

    fn write_product(
        &mut self,
        results: &[ResultValue],
        value: Self::Value,
    ) -> Result<(), Self::Error>
    where
        Self::Value: ProductValue,
    {
        if results.is_empty() {
            return Ok(());
        }
        if results.len() == 1 {
            self.write_result(results[0], value)?;
        } else if let Some(components) = value.as_product() {
            for (result, v) in results.iter().zip(components.iter()) {
                self.write_result(*result, v.clone())?;
            }
        } else {
            self.write_result(results[0], value)?;
        }
        Ok(())
    }
}

/// Concrete (cursor-stack) domain.
pub trait ConcreteEnv: Env {
    type Cursor;

    /// Take the pending yield value, if any.
    fn take_pending_yield(&mut self) -> Option<Self::Value>;
}

/// Abstract (worklist fixpoint) domain.
///
/// `Ext = Infallible` proves at the type level that abstract execution never
/// produces cursor push/pop events.
pub trait AbstractEnv: Env<Ext = Infallible> {
    /// Enqueue a block for (re-)analysis with the given entry arguments.
    fn enqueue_block(&mut self, block: Block, args: Vec<Self::Value>);

    /// Record a return/yield value from the current function.
    fn record_return(&mut self, v: Self::Value) -> Result<(), Self::Error>;

    /// The function currently being analyzed.
    fn current_function(&self) -> SpecializedFunction;
}
