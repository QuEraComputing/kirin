use kirin::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "$call {target}({args}) -> {res:type}")]
pub struct Call<T: CompileTimeValue> {
    target: Symbol,
    args: Vec<SSAValue>,
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Call<T> {
    pub fn target(&self) -> Symbol {
        self.target
    }

    pub fn args(&self) -> &[SSAValue] {
        &self.args
    }

    pub fn result(&self) -> ResultValue {
        self.res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin::ir::{
        HasArguments, HasBlocks, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
        IsSpeculatable, IsTerminator, TestSSAValue,
    };
    use kirin_test_types::UnitType;

    fn make_call(num_args: usize) -> Call<UnitType> {
        Call {
            target: Symbol::from(42usize),
            args: (0..num_args).map(|i| TestSSAValue(i).into()).collect(),
            res: TestSSAValue(100).into(),
            marker: std::marker::PhantomData,
        }
    }

    #[test]
    fn not_terminator() {
        assert!(!make_call(0).is_terminator());
    }

    #[test]
    fn not_pure() {
        assert!(!make_call(0).is_pure());
    }

    #[test]
    fn not_constant() {
        assert!(!make_call(0).is_constant());
    }

    #[test]
    fn not_speculatable() {
        assert!(!make_call(0).is_speculatable());
    }

    #[test]
    fn has_one_result() {
        assert_eq!(make_call(0).results().count(), 1);
    }

    #[test]
    fn no_successors() {
        assert_eq!(make_call(0).successors().count(), 0);
    }

    #[test]
    fn no_blocks() {
        assert_eq!(make_call(0).blocks().count(), 0);
    }

    #[test]
    fn no_regions() {
        assert_eq!(make_call(0).regions().count(), 0);
    }

    #[test]
    fn target_accessor() {
        let call = make_call(0);
        assert_eq!(call.target(), Symbol::from(42usize));
    }

    #[test]
    fn args_accessor_empty() {
        assert_eq!(make_call(0).args().len(), 0);
    }

    #[test]
    fn args_accessor_multiple() {
        let call = make_call(3);
        assert_eq!(call.args().len(), 3);
        assert_eq!(call.args()[0], TestSSAValue(0).into());
        assert_eq!(call.args()[1], TestSSAValue(1).into());
        assert_eq!(call.args()[2], TestSSAValue(2).into());
    }

    #[test]
    fn result_accessor() {
        let call = make_call(0);
        assert_eq!(call.result(), TestSSAValue(100).into());
    }

    #[test]
    fn arguments_matches_args() {
        let call = make_call(2);
        let args: Vec<_> = call.arguments().copied().collect();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], TestSSAValue(0).into());
        assert_eq!(args[1], TestSSAValue(1).into());
    }

    #[test]
    fn clone_eq() {
        let call = make_call(2);
        assert_eq!(call, call.clone());
    }

    #[test]
    fn different_calls_not_equal() {
        assert_ne!(make_call(0), make_call(1));
    }

    #[test]
    fn debug_contains_call() {
        let dbg = format!("{:?}", make_call(0));
        assert!(dbg.contains("Call"), "debug should contain 'Call': {dbg}");
    }
}
