use kirin_ir::{CompileStage, CompileStageInfo, Pipeline, ResultValue, SSAValue};

use crate::InterpretControl;

/// Minimal state contract for interpreter implementations.
///
/// Provides SSA value read/write only. The associated `Control` type
/// determines which control flow actions are available — concrete
/// interpreters use [`crate::ConcreteControl`] while abstract interpreters
/// use [`crate::AbstractControl`].
///
/// Call semantics are inherent on each interpreter type
/// (e.g. [`crate::StackInterpreter::call`],
/// [`crate::AbstractInterpreter::analyze`]).
pub trait Interpreter: Sized {
    type Value;
    type Error;
    type Control: InterpretControl<Self::Value>;
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
