mod interpret_impl;

use kirin::prelude::*;

pub mod interpreter10;
pub mod interpreter2;
pub mod interpreter4;
pub mod interpreter5;
pub mod interpreter6;
pub mod interpreter7;
pub mod interpreter8;
pub mod interpreter9;

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
