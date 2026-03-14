use kirin::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "{res:name} = {.bind} {target} captures({captures}) -> {res:type}")]
pub struct Bind<T: CompileTimeValue> {
    target: Symbol,
    captures: Vec<SSAValue>,
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
