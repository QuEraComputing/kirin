use crate::{Continuation, Interpreter};
use kirin_ir::{ResultValue, SSAValue};

/// Convenience methods for common interpreter patterns.
///
/// Blanket-implemented for all `I: Interpreter<'ir>`. Dialect authors
/// get these methods for free — no extra import beyond the prelude.
pub trait InterpreterExt<'ir>: Interpreter<'ir> {
    /// Read two SSA values, apply `op`, write the result, continue.
    fn binary_op<F>(
        &mut self,
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value, Self::Value) -> Self::Value;

    /// Read one SSA value, apply `op`, write the result, continue.
    fn unary_op<F>(
        &mut self,
        operand: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value) -> Self::Value;

    /// Read two SSA values, apply fallible `op`, write the result, continue.
    fn try_binary_op<F>(
        &mut self,
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value, Self::Value) -> Result<Self::Value, Self::Error>;
}

impl<'ir, I: Interpreter<'ir>> InterpreterExt<'ir> for I {
    fn binary_op<F>(
        &mut self,
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value, Self::Value) -> Self::Value,
    {
        let a = self.read(lhs)?;
        let b = self.read(rhs)?;
        self.write(result, op(a, b))?;
        Ok(Continuation::Continue)
    }

    fn unary_op<F>(
        &mut self,
        operand: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value) -> Self::Value,
    {
        let a = self.read(operand)?;
        self.write(result, op(a))?;
        Ok(Continuation::Continue)
    }

    fn try_binary_op<F>(
        &mut self,
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Continuation<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value, Self::Value) -> Result<Self::Value, Self::Error>,
    {
        let a = self.read(lhs)?;
        let b = self.read(rhs)?;
        let v = op(a, b)?;
        self.write(result, v)?;
        Ok(Continuation::Continue)
    }
}
