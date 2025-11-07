use crate::{
    comptime::CompileTimeValue,
    ir::{Instruction, ResultValue},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Constant<T: CompileTimeValue>(pub T, ResultValue);

impl<T: CompileTimeValue> Instruction for Constant<T> {
    type ResultIterator = std::iter::Once<crate::ir::ResultValue>;
    fn results(&self) -> Self::ResultIterator {
        std::iter::once(self.1)
    }
}
