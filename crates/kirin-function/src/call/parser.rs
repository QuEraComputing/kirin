use kirin::parsers::HasDialectEmitIR;
use kirin::parsers::SymbolName;
use kirin::prelude::*;

use super::{Call, CallNamed};

#[derive(Clone, PartialEq)]
pub enum CallTargetAST<'t> {
    Named(SymbolName<'t>),
}

#[derive(Clone, PartialEq)]
pub struct CallAST<'t, T>
where
    T: HasParser<'t>,
{
    target: CallTargetAST<'t>,
    args: Vec<kirin::parsers::SSAValue<'t, <T as HasParser<'t>>::Output>>,
    results: Vec<kirin::parsers::ResultValue<'t, <T as HasParser<'t>>::Output>>,
}

impl<'t, T> HasDialectParser<'t> for Call<T>
where
    T: CompileTimeValue + HasParser<'t> + 't,
{
    type Output<TypeOutput, LanguageOutput>
        = CallAST<'t, T>
    where
        TypeOutput: Clone + PartialEq + 't,
        LanguageOutput: Clone + PartialEq + 't;

    fn namespaced_parser<I, TypeOutput, LanguageOutput>(
        _language: RecursiveParser<'t, I, LanguageOutput>,
        _namespace: &[&'static str],
    ) -> BoxedParser<'t, I, Self::Output<TypeOutput, LanguageOutput>>
    where
        I: TokenInput<'t>,
        TypeOutput: Clone + PartialEq + 't,
        LanguageOutput: Clone + PartialEq + 't,
    {
        result_name_list()
            .then(call_keyword::<I>("named"))
            .then(symbol())
            .then(call_args_parser::<I, T>())
            .then(call_result_types_parser::<I, T>())
            .map(|((((result_names, _), target), args), result_types)| {
                call_ast_from_parts(
                    result_names,
                    CallTargetAST::Named(target),
                    args,
                    result_types,
                )
            })
            .or(call_keyword::<I>("named")
                .ignore_then(symbol())
                .then(call_args_parser::<I, T>())
                .then(call_result_types_parser::<I, T>())
                .map(|((target, args), result_types)| {
                    call_ast_from_parts(
                        Vec::new(),
                        CallTargetAST::Named(target),
                        args,
                        result_types,
                    )
                }))
            .boxed()
    }
}

fn call_ast_from_parts<'t, T>(
    result_names: Vec<kirin::parsers::Spanned<&'t str>>,
    target: CallTargetAST<'t>,
    args: Vec<kirin::parsers::SSAValue<'t, <T as HasParser<'t>>::Output>>,
    result_types: Option<Vec<kirin::parsers::TypeofSSAValue<<T as HasParser<'t>>::Output>>>,
) -> CallAST<'t, T>
where
    T: HasParser<'t>,
{
    let result_types = result_types.unwrap_or_default();
    let results = result_names
        .into_iter()
        .enumerate()
        .map(|(result_index, name)| kirin::parsers::ResultValue {
            name,
            ty: result_types.get(result_index).map(|ty| ty.ty.clone()),
            result_index,
        })
        .collect();
    CallAST {
        target,
        args,
        results,
    }
}

fn call_args_parser<'t, I, T>()
-> BoxedParser<'t, I, Vec<kirin::parsers::SSAValue<'t, <T as HasParser<'t>>::Output>>>
where
    I: TokenInput<'t>,
    T: HasParser<'t> + 't,
{
    ssa_value::<_, T>()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .boxed()
}

fn call_result_types_parser<'t, I, T>()
-> BoxedParser<'t, I, Option<Vec<kirin::parsers::TypeofSSAValue<<T as HasParser<'t>>::Output>>>>
where
    I: TokenInput<'t>,
    T: HasParser<'t> + 't,
{
    just(Token::Arrow)
        .ignore_then(
            typeof_ssa::<_, T>()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>(),
        )
        .or_not()
        .boxed()
}

fn call_keyword<'t, I>(name: &'static str) -> BoxedParser<'t, I, ()>
where
    I: TokenInput<'t>,
{
    identifier("call")
        .then_ignore(just(Token::Dot))
        .then_ignore(identifier(name))
        .ignored()
        .boxed()
}

impl<'t, T, Language, LanguageOutput> HasDialectEmitIR<'t, Language, LanguageOutput> for Call<T>
where
    T: CompileTimeValue + HasParser<'t> + 't,
    Language: Dialect<Type = T>,
    LanguageOutput: Clone + PartialEq + 't,
    <T as HasParser<'t>>::Output: EmitIR<Language, Output = T>,
{
    fn emit_output<TypeOutput, EmitLanguageOutput>(
        output: &<Self as HasDialectParser<'t>>::Output<TypeOutput, LanguageOutput>,
        ctx: &mut EmitContext<'_, Language>,
        _emit_language_output: &EmitLanguageOutput,
    ) -> Result<Self, EmitError>
    where
        TypeOutput: Clone + PartialEq + 't,
        EmitLanguageOutput: for<'ctx> Fn(
            &LanguageOutput,
            &mut EmitContext<'ctx, Language>,
        ) -> Result<Statement, EmitError>,
    {
        let args = output
            .args
            .iter()
            .map(|arg| arg.emit(ctx))
            .collect::<Result<Vec<_>, _>>()?;
        let results = output
            .results
            .iter()
            .map(|result| result.emit(ctx))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(match &output.target {
            CallTargetAST::Named(target) => Call::Named(CallNamed {
                target: target.emit(ctx)?,
                args,
                results,
                compile_stage: None,
                marker: std::marker::PhantomData,
            }),
        })
    }
}
