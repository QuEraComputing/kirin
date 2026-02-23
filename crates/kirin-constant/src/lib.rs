#[cfg(feature = "interpret")]
mod interpret_impl;

use kirin::prelude::*;

#[derive(HasParser, PrettyPrint)]
#[chumsky(format = "{result:name} = constant {value} -> {result:type}")]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(constant, fn = new, type = Ty)]
pub struct Constant<T: CompileTimeValue + Typeof<Ty> + PrettyPrint, Ty: CompileTimeValue + Default> {
    #[kirin(into)]
    pub value: T,
    #[kirin(type = value.type_of())]
    pub result: ResultValue,
    #[kirin(default)]
    pub marker: std::marker::PhantomData<Ty>,
}
