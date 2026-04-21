use kirin_ir::{ResultValue, SSAValue};

use crate::control::Control;
use crate::env::Env;

pub trait EnvExt: Env {
    fn binary_op<F>(
        &mut self,
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value, Self::Value) -> Self::Value;

    fn unary_op<F>(
        &mut self,
        operand: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value) -> Self::Value;

    fn try_binary_op<F>(
        &mut self,
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value, Self::Value) -> Result<Self::Value, Self::Error>;
}

impl<E: Env> EnvExt for E {
    fn binary_op<F>(
        &mut self,
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value, Self::Value) -> Self::Value,
    {
        let a = self.read(lhs)?;
        let b = self.read(rhs)?;
        self.write_result(result, op(a, b))?;
        Ok(Control::Advance)
    }

    fn unary_op<F>(
        &mut self,
        operand: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value) -> Self::Value,
    {
        let a = self.read(operand)?;
        self.write_result(result, op(a))?;
        Ok(Control::Advance)
    }

    fn try_binary_op<F>(
        &mut self,
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        op: F,
    ) -> Result<Control<Self::Value, Self::Ext>, Self::Error>
    where
        F: FnOnce(Self::Value, Self::Value) -> Result<Self::Value, Self::Error>,
    {
        let a = self.read(lhs)?;
        let b = self.read(rhs)?;
        let v = op(a, b)?;
        self.write_result(result, v)?;
        Ok(Control::Advance)
    }
}
