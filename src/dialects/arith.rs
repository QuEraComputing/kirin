use kirin_ir::*;

#[derive(Clone, Debug, PartialEq, Statement)]
pub enum ArithInstruction {
    Add(SSAValue, SSAValue, ResultValue),
    Sub(SSAValue, SSAValue, ResultValue),
    Mul(SSAValue, SSAValue, ResultValue),
    Div(SSAValue, SSAValue, ResultValue),
}
