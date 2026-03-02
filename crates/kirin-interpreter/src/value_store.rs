use kirin_ir::{ResultValue, SSAValue};

/// Value read/write operations for SSA bindings.
///
/// This is the minimal storage interface that all interpreter implementations
/// share. Dialect `Interpretable` impls use this to read operands and write
/// results.
pub trait ValueStore {
    /// The value type manipulated by this interpreter.
    ///
    /// Values should be cheap to clone — typically pointer-sized handles,
    /// small enums, or wrappers around `Arc`/`Rc` for heavier data.
    type Value: Clone;
    type Error;

    /// Returns a cloned copy of the bound value.
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;

    /// Bind a result to a value.
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;

    /// Bind an SSA value directly (e.g. block arguments).
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;
}
