use kirin::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(terminator, fn, type = T)]
#[chumsky(format = "ret {value}")]
pub struct Return<T: CompileTimeValue + Default> {
    value: SSAValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
