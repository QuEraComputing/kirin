use std::fmt;

use kirin_ir::{CompileStage, CompileStageInfo, Pipeline, ResultValue, SSAValue};

/// Minimal state contract for interpreter implementations.
///
/// Provides SSA value read/write only. The associated `Ext` type
/// determines which extra continuation variants are available — concrete
/// interpreters use [`crate::ConcreteExt`] while abstract interpreters
/// use [`std::convert::Infallible`].
///
/// Call semantics are inherent on each interpreter type
/// (e.g. [`crate::StackInterpreter::call`],
/// [`crate::AbstractInterpreter::analyze`]).
pub trait Interpreter: Sized {
    type Value;
    type Error;
    type Ext: fmt::Debug;
    type StageInfo: CompileStageInfo;

    /// Returns a reference to the bound value without cloning.
    fn read_ref(&self, value: SSAValue) -> Result<&Self::Value, Self::Error>;

    /// Returns a cloned copy of the bound value.
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>
    where
        Self::Value: Clone,
    {
        self.read_ref(value).cloned()
    }

    /// Bind a result to a value.
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;

    /// Reference to the IR pipeline.
    fn pipeline(&self) -> &Pipeline<Self::StageInfo>;

    /// The currently active compilation stage.
    fn active_stage(&self) -> CompileStage;
}
