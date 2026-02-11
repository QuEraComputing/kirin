use kirin::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
#[chumsky(format = "{res:name} = lambda {name} captures({captures}) {body} -> {res:type}")]
pub struct Lambda<T: CompileTimeValue + Default> {
    name: Symbol,
    captures: Vec<SSAValue>,
    body: Region,
    #[kirin(type = T::default())]
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
