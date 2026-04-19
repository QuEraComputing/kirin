use std::marker::PhantomData;

use kirin_interpreter::ProductValue;
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageInfo, StageMeta, Symbol,
};

use crate::error::InterpreterError;

// ---------------------------------------------------------------------------
// Mode discriminants
// ---------------------------------------------------------------------------

/// Marker for concrete (cursor-stack) interpreter mode.
pub struct ConcreteMode<C>(PhantomData<C>);

/// Marker for abstract (worklist fixpoint) interpreter mode.
pub struct AbstractMode<C>(PhantomData<C>);

// ---------------------------------------------------------------------------
// Env — unified domain trait
// ---------------------------------------------------------------------------

/// Unified domain trait for interpreter-10.
///
/// `type Mode` is the key discriminant:
/// - `ConcreteMode<C>` — concrete cursor-stack interpreter
/// - `AbstractMode<C>` — abstract worklist fixpoint interpreter
///
/// Dialect semantics implement `Interpretable<E>` generically (`impl<E: Env>`) for
/// pure ops, or with `E: Env<Mode = ConcreteMode<C>>` / `AbstractMode<C>` for
/// mode-specific ops (e.g. SCF).
pub trait Env {
    type Mode;
    type Value: Clone;
    type Ext;
    type Error: From<InterpreterError>;
    type Stages: StageMeta;

    fn current_stage(&self) -> CompileStage;
    fn pipeline(&self) -> &Pipeline<Self::Stages>;

    fn read(&self, ssa: SSAValue) -> Result<Self::Value, Self::Error>;
    fn write_result(&mut self, r: ResultValue, v: Self::Value) -> Result<(), Self::Error>;
    fn write_ssa(&mut self, ssa: SSAValue, v: Self::Value) -> Result<(), Self::Error>;

    // -- Provided helpers ----------------------------------------------------

    fn read_many(&self, ssas: &[SSAValue]) -> Result<Vec<Self::Value>, Self::Error> {
        ssas.iter().map(|&ssa| self.read(ssa)).collect()
    }

    /// Retrieve the stage info for dialect `L` at stage `id`, or error with
    /// `MissingEntry`. Reduces the repeated `.ok_or_else(|| ... MissingEntry)?`
    /// pattern across cursor execute impls.
    fn require_stage<L: Dialect>(&self, id: CompileStage) -> Result<&StageInfo<L>, Self::Error>
    where
        Self::Stages: HasStageInfo<L>,
    {
        self.pipeline()
            .stage(id)
            .and_then(|s| s.try_stage_info())
            .ok_or_else(|| Self::Error::from(InterpreterError::MissingEntry))
    }

    fn stage_info_for<L: Dialect>(&self, id: CompileStage) -> Option<&StageInfo<L>>
    where
        Self::Stages: HasStageInfo<L>,
    {
        self.pipeline().stage(id)?.try_stage_info()
    }

    fn resolve_function_for<L: Dialect>(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, Self::Error>
    where
        Self::Stages: HasStageInfo<L>,
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

    fn write_results(
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

// ---------------------------------------------------------------------------
// AbstractEnv — additional protocol for worklist-based abstract interpreters
// ---------------------------------------------------------------------------

pub trait AbstractEnv: Env {
    fn enqueue_block(&mut self, block: Block, args: Vec<Self::Value>);
    fn record_return(&mut self, v: Self::Value) -> Result<(), Self::Error>;
    fn current_function(&self) -> SpecializedFunction;
}
