use chumsky::prelude::*;
use kirin_ir::{Function, SpecializedFunction, StagedFunction};

use crate::traits::{BoxedParser, HasParser, TokenInput};

fn unsupported_handle_parser<'t, I, T>(label: &'static str) -> BoxedParser<'t, I, T>
where
    I: TokenInput<'t>,
    T: Clone + PartialEq + 't,
{
    empty()
        .try_map(move |(), span| {
            Err::<T, _>(Rich::custom(
                span,
                format!("{label} handles cannot be parsed from text"),
            ))
        })
        .labelled(label)
        .boxed()
}

impl<'t> HasParser<'t> for Function {
    type Output = Function;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        unsupported_handle_parser("function")
    }
}

impl<'t> HasParser<'t> for StagedFunction {
    type Output = StagedFunction;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        unsupported_handle_parser("staged function")
    }
}

impl<'t> HasParser<'t> for SpecializedFunction {
    type Output = SpecializedFunction;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        unsupported_handle_parser("specialized function")
    }
}
