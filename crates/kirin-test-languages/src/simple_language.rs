use crate::{SimpleType, Value};
use kirin_ir::{Dialect, Region, ResultValue, SSAKind, SSAValue};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[kirin(fn, type = SimpleType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
pub enum SimpleLanguage {
    #[cfg_attr(
        feature = "parser",
        chumsky(format = "{2:name} = add {0}, {1} -> {2:type}")
    )]
    Add(
        SSAValue,
        SSAValue,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    #[cfg_attr(
        feature = "parser",
        chumsky(format = "{1:name} = constant {0} -> {1:type}")
    )]
    Constant(
        #[kirin(into)] Value,
        #[kirin(type = SimpleType::F64)] ResultValue,
    ),
    #[kirin(terminator)]
    #[cfg_attr(feature = "parser", chumsky(format = "return {0}"))]
    Return(SSAValue),
    #[cfg_attr(feature = "parser", chumsky(format = "{1} = func {0}"))]
    Function(Region, #[kirin(type = SimpleType::F64)] ResultValue),
}
