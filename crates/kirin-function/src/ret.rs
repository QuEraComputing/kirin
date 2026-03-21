use kirin::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(terminator, builders, type = T)]
#[chumsky(format = "$ret {value}")]
pub struct Return<T: CompileTimeValue> {
    pub(crate) value: SSAValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin::ir::{
        HasArguments, HasBlocks, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
        IsSpeculatable, IsTerminator, TestSSAValue,
    };

    #[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
    struct UnitTy;

    impl std::fmt::Display for UnitTy {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "unit")
        }
    }

    fn make_return() -> Return<UnitTy> {
        Return {
            value: TestSSAValue(0).into(),
            marker: std::marker::PhantomData,
        }
    }

    #[test]
    fn is_terminator() {
        assert!(make_return().is_terminator());
    }

    #[test]
    fn not_pure() {
        assert!(!make_return().is_pure());
    }

    #[test]
    fn not_constant() {
        assert!(!make_return().is_constant());
    }

    #[test]
    fn not_speculatable() {
        assert!(!make_return().is_speculatable());
    }

    #[test]
    fn has_one_argument() {
        let ret = make_return();
        let args: Vec<_> = ret.arguments().copied().collect();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], TestSSAValue(0).into());
    }

    #[test]
    fn no_results() {
        assert_eq!(make_return().results().count(), 0);
    }

    #[test]
    fn no_successors() {
        assert_eq!(make_return().successors().count(), 0);
    }

    #[test]
    fn no_blocks() {
        assert_eq!(make_return().blocks().count(), 0);
    }

    #[test]
    fn no_regions() {
        assert_eq!(make_return().regions().count(), 0);
    }

    #[test]
    fn clone_eq() {
        let ret = make_return();
        assert_eq!(ret, ret.clone());
    }

    #[test]
    fn debug_contains_return() {
        let dbg = format!("{:?}", make_return());
        assert!(
            dbg.contains("Return"),
            "debug should contain 'Return': {dbg}"
        );
    }

    #[test]
    fn value_field() {
        let ret = make_return();
        assert_eq!(ret.value, TestSSAValue(0).into());
    }
}
