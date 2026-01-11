use super::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// an SSA value reference, e.g
/// ```ignore
/// %value
/// ```
///
/// with optional type annotation
/// ```ignore
/// %value: type
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SSAValue<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>> {
    pub name: &'src str,
    pub ty: Option<<Language::TypeLattice as WithChumskyParser<'tokens, 'src, Language>>::Output>,
    pub span: SimpleSpan,
}

impl<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>>
    SSAValue<'tokens, 'src, Language>
{
    pub fn new(
        name: &'src str,
        ty: Option<<Language::TypeLattice as WithChumskyParser<'tokens, 'src, Language>>::Output>,
        span: SimpleSpan,
    ) -> Self {
        Self { name, ty, span }
    }
}

/// the value on left-hand side of an SSA assignment, i.e
/// ```ignore
/// %result = ...
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ResultValue<'src> {
    pub name: &'src str,
    pub span: SimpleSpan,
}

impl<'src> ResultValue<'src> {
    pub fn new(name: &'src str, span: SimpleSpan) -> Self {
        Self { name, span }
    }
}

/// the type of an SSA value, used when a type annotation is present in a place other than
/// the standard SSA value syntax, e.g
///
/// given the syntax specification of `add` as
///
/// ```ignore
/// add {lhs} {rhs} -> {result:type}
/// ```
///
/// when parsing an instruction like
///
/// ```ignore
/// add %a %b -> bool
/// ```
///
/// `%a` and `%b` are parsed as `SSAValue` with type `None`,
/// and we are expecting the type of `%result` to be `bool`,
/// however, `%result` is not allowed to be part of the syntax specification,
/// so `-> bool` is parsed as `TypeofSSAValue`.
///
/// Later, when constructing the IR, we can assign the type `bool` to the SSA value `%result`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeofSSAValue<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>> {
    pub ty: <Language::TypeLattice as WithChumskyParser<'tokens, 'src, Language>>::Output,
    pub span: SimpleSpan,
}

impl<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>>
    TypeofSSAValue<'tokens, 'src, Language>
{
    pub fn new(
        ty: <Language::TypeLattice as WithChumskyParser<'tokens, 'src, Language>>::Output,
        span: SimpleSpan,
    ) -> Self {
        Self { ty, span }
    }
}

/// parse a `%<name>` into `ast::NameofSSAValue`
#[derive(Debug, Clone, PartialEq)]
pub struct NameofSSAValue<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>> {
    pub name: &'src str,
    pub span: SimpleSpan,
    marker: std::marker::PhantomData<&'tokens Language>,
}

impl<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>>
    NameofSSAValue<'tokens, 'src, Language>
{
    pub fn new(name: &'src str, span: SimpleSpan) -> Self {
        Self {
            name,
            span,
            marker: std::marker::PhantomData,
        }
    }
}

/// parse a `%<name>` into `ast::SSAValue`
pub fn ssa<'tokens, 'src: 'tokens, I, Language>()
-> impl ChumskyParser<'tokens, 'src, I, SSAValue<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src>,
{
    select! {
        Token::SSAValue(name) = e => SSAValue {
            name,
            ty: None,
            span: e.span(),
        }
    }
    .labelled("SSAValue")
}

/// parse a `%<name>` into `ast::ResultValue`
pub fn result_value<'tokens, 'src: 'tokens, I>()
-> impl ChumskyParser<'tokens, 'src, I, ResultValue<'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
{
    select! {
        Token::SSAValue(name) = e => ResultValue {
            name,
            span: e.span(),
        }
    }
    .labelled("ResultValue")
}

/// parse a `%<name> : <type>` into `ast::SSAValue` with type
pub fn ssa_with_type<'tokens, 'src: 'tokens, I, Language>()
-> impl ChumskyParser<'tokens, 'src, I, SSAValue<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src>,
{
    ssa()
        .then_ignore(just(Token::Colon))
        .then(Language::TypeLattice::parser())
        .map(|(mut ssavalue, ty)| {
            ssavalue.ty = Some(ty);
            ssavalue
        })
        .labelled("SSAValue with type")
}

pub fn typeof_ssa<'tokens, 'src: 'tokens, I, Language>()
-> impl ChumskyParser<'tokens, 'src, I, TypeofSSAValue<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src>,
{
    Language::TypeLattice::parser()
        .map_with(|ty, extra| TypeofSSAValue {
            ty,
            span: extra.span(),
        })
        .labelled("TypeofSSAValue")
}

pub fn nameof_ssa<'tokens, 'src: 'tokens, I, Language>()
-> impl ChumskyParser<'tokens, 'src, I, NameofSSAValue<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src> + 'tokens,
{
    select! {
        Token::SSAValue(name) = e => NameofSSAValue::new(name, e.span())
    }
    .labelled("NameofSSAValue")
}
