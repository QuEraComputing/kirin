use kirin::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
#[chumsky(format = "{res:name} = bind {target} captures({captures}) -> {res:type}")]
pub struct Bind<T: CompileTimeValue + Default> {
    target: Symbol,
    captures: Vec<SSAValue>,
    #[kirin(type = T::default())]
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
