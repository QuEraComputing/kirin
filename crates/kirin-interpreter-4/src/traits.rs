use kirin_ir::{CompileStage, Pipeline, ResultValue, SSAValue};

/// Any stateful component that can consume effects.
pub trait Machine {
    type Effect;
    type Error;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error>;
}

/// SSA value read/write.
pub trait ValueStore {
    type Value: Clone;
    type Error;

    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;

    /// Read multiple values. Provided default.
    fn read_many(&self, values: &[SSAValue]) -> Result<Vec<Self::Value>, Self::Error> {
        values.iter().map(|v| self.read(*v)).collect()
    }
}

/// Pipeline and stage identity.
pub trait PipelineAccess {
    type StageInfo;

    fn pipeline(&self) -> &Pipeline<Self::StageInfo>;
    fn current_stage(&self) -> CompileStage;
}

/// Statement-level operational semantics.
///
/// Dialect authors implement this to define how a statement executes.
/// `&mut I` provides mutation access (values, machine state, execution seeds).
/// The return type provides an effect channel for deferred control flow.
pub trait Interpretable<I: Interpreter> {
    type Effect;
    type Error;

    fn interpret(&self, interp: &mut I) -> Result<Self::Effect, Self::Error>;
}

/// Complete interpreter: Machine + ValueStore + PipelineAccess.
///
/// This is automatically implemented for anything that implements all three sub-traits
/// with compatible associated types.
pub trait Interpreter:
    Machine + ValueStore<Error = <Self as Machine>::Error> + PipelineAccess
{
}

// Blanket impl
impl<T> Interpreter for T where
    T: Machine + ValueStore<Error = <T as Machine>::Error> + PipelineAccess
{
}
