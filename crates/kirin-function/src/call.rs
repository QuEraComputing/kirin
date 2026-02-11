use kirin::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
#[chumsky(format = "{res:name} = call {target}({args}) -> {res:type}")]
pub struct Call<T: CompileTimeValue + Default> {
    target: Symbol,
    args: Vec<SSAValue>,
    #[kirin(type = T::default())]
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
