use kirin_ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Instruction)]
#[kirin(is_constant = true)]
pub struct Constant<T: CompileTimeValue>(pub T, ResultValue);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_results() {
        let const_instr = Constant(42u32, TestSSAValue(1).into());
        let results = const_instr.results().collect::<Vec<_>>();
        assert_eq!(results, vec![&(TestSSAValue(1).into())]);
    }
}
