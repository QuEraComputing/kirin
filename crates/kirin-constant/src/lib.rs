mod interpret_impl;

use kirin::prelude::*;

pub mod interpreter2;

#[cfg(test)]
mod tests;

#[derive(HasParser, PrettyPrint)]
#[chumsky(format = "$constant {value} -> {result:type}")]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(constant, pure, builders = new, type = Ty)]
pub struct Constant<T: CompileTimeValue + Typeof<Ty> + PrettyPrint, Ty: CompileTimeValue> {
    #[kirin(into)]
    pub value: T,
    #[kirin(type = value.type_of())]
    pub result: ResultValue,
    #[kirin(default)]
    pub marker: std::marker::PhantomData<Ty>,
}
