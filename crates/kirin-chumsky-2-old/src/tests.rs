use chumsky::{input::Input, input::Stream, prelude::*, Parser};
use kirin_lexer::Logos;

use crate::{
    parsers,
    BoxedParser, RecursiveParser, TokenInput, WithChumskyParser, WithRecursiveChumskyParser,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SimpleTy {
    F64,
}

impl kirin_ir::Lattice for SimpleTy {
    fn join(&self, _other: &Self) -> Self {
        SimpleTy::F64
    }

    fn meet(&self, _other: &Self) -> Self {
        SimpleTy::F64
    }

    fn is_subseteq(&self, _other: &Self) -> bool {
        true
    }
}

impl kirin_ir::FiniteLattice for SimpleTy {
    fn bottom() -> Self {
        SimpleTy::F64
    }

    fn top() -> Self {
        SimpleTy::F64
    }
}

impl kirin_ir::TypeLattice for SimpleTy {}

impl<'tokens, 'src: 'tokens> WithChumskyParser<'tokens, 'src> for SimpleTy {
    type Output = SimpleTy;

    fn parser<I>() -> crate::BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        select! { kirin_lexer::Token::Identifier("f64") => SimpleTy::F64 }.boxed()
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SimpleLang<'tokens, 'src: 'tokens> {
    Add {
        lhs: crate::SSAValue<'tokens, 'src, SimpleLang<'tokens, 'src>>,
        rhs: crate::SSAValue<'tokens, 'src, SimpleLang<'tokens, 'src>>,
        result: kirin_ir::ResultValue,
        result_ty: crate::TypeofSSAValue<'tokens, 'src, SimpleLang<'tokens, 'src>>,
    },
    Function {
        name: String,
        args: Vec<crate::SSAValue<'tokens, 'src, SimpleLang<'tokens, 'src>>>,
        ret: crate::TypeofSSAValue<'tokens, 'src, SimpleLang<'tokens, 'src>>,
        body: crate::Region<'tokens, 'src, SimpleLang<'tokens, 'src>>,
    },
}

impl<'tokens, 'src> SimpleLang<'tokens, 'src> {
    fn to_result(name: &str) -> kirin_ir::ResultValue {
        let id = name.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize));
        kirin_ir::ResultValue::from(kirin_ir::TestSSAValue(id))
    }
}

impl<'a> kirin_ir::HasArguments<'a> for SimpleLang<'a, 'a> {
    type Iter = std::iter::Empty<&'a kirin_ir::SSAValue>;
    fn arguments(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> kirin_ir::HasArgumentsMut<'a> for SimpleLang<'a, 'a> {
    type IterMut = std::iter::Empty<&'a mut kirin_ir::SSAValue>;
    fn arguments_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> kirin_ir::HasResults<'a> for SimpleLang<'a, 'a> {
    type Iter = std::slice::Iter<'a, kirin_ir::ResultValue>;
    fn results(&'a self) -> Self::Iter {
        match self {
            SimpleLang::Add { result, .. } => std::slice::from_ref(result).iter(),
            SimpleLang::Function { .. } => [].iter(),
        }
    }
}

impl<'a> kirin_ir::HasResultsMut<'a> for SimpleLang<'a, 'a> {
    type IterMut = std::slice::IterMut<'a, kirin_ir::ResultValue>;
    fn results_mut(&'a mut self) -> Self::IterMut {
        match self {
            SimpleLang::Add { result, .. } => std::slice::from_mut(result).iter_mut(),
            SimpleLang::Function { .. } => [].iter_mut(),
        }
    }
}

impl<'a> kirin_ir::HasBlocks<'a> for SimpleLang<'a, 'a> {
    type Iter = std::iter::Empty<&'a kirin_ir::Block>;
    fn blocks(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> kirin_ir::HasBlocksMut<'a> for SimpleLang<'a, 'a> {
    type IterMut = std::iter::Empty<&'a mut kirin_ir::Block>;
    fn blocks_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> kirin_ir::HasSuccessors<'a> for SimpleLang<'a, 'a> {
    type Iter = std::iter::Empty<&'a kirin_ir::Successor>;
    fn successors(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> kirin_ir::HasSuccessorsMut<'a> for SimpleLang<'a, 'a> {
    type IterMut = std::iter::Empty<&'a mut kirin_ir::Successor>;
    fn successors_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> kirin_ir::HasRegions<'a> for SimpleLang<'a, 'a> {
    type Iter = std::iter::Empty<&'a kirin_ir::Region>;
    fn regions(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> kirin_ir::HasRegionsMut<'a> for SimpleLang<'a, 'a> {
    type IterMut = std::iter::Empty<&'a mut kirin_ir::Region>;
    fn regions_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl kirin_ir::IsTerminator for SimpleLang {
    fn is_terminator(&self) -> bool {
        false
    }
}

impl kirin_ir::IsConstant for SimpleLang {
    fn is_constant(&self) -> bool {
        false
    }
}

impl kirin_ir::IsPure for SimpleLang {
    fn is_pure(&self) -> bool {
        true
    }
}

impl<'tokens, 'src> kirin_ir::Dialect for SimpleLang<'tokens, 'src> {
    type TypeLattice = SimpleTy;
}

impl<'tokens, 'src: 'tokens> WithRecursiveChumskyParser<'tokens, 'src, SimpleLang<'tokens, 'src>>
    for SimpleLang<'tokens, 'src>
{
    type Output = SimpleLang<'tokens, 'src>;

    fn recursive<I>(
        language: RecursiveParser<'tokens, 'src, I, Self::Output>,
    ) -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        let add_parser = select! { kirin_lexer::Token::Identifier("add") => () }
            .ignore_then(parsers::ssa::<I, SimpleLang>())
            .then(parsers::ssa::<I, SimpleLang>())
            .then_ignore(select! { kirin_lexer::Token::Arrow => () }.ignored())
            .then(parsers::typeof_ssa::<I, SimpleLang>())
            .map(|((lhs, rhs), result_ty)| SimpleLang::Add {
                lhs: SimpleLang::to_ssa(lhs.name),
                rhs: SimpleLang::to_ssa(rhs.name),
                result: SimpleLang::to_result("result"),
                result_ty: result_ty.ty,
            });

        let fn_parser = select! { kirin_lexer::Token::Identifier("fn") => () }
            .ignore_then(select! { kirin_lexer::Token::Identifier(name) => name.to_string() })
            .then_ignore(select! { kirin_lexer::Token::LParen => () }.ignored())
            .then(
                parsers::ssa_with_type::<I, SimpleLang>()
                    .repeated()
                    .collect::<Vec<crate::SSAValue<'tokens, 'src, SimpleLang>>>(),
            )
            .then_ignore(select! { kirin_lexer::Token::RParen => () }.ignored())
            .then_ignore(select! { kirin_lexer::Token::Arrow => () }.ignored())
            .then(parsers::typeof_ssa::<I, SimpleLang>())
            .then(parsers::region::<I, SimpleLang>(language.clone()).or_not())
            .map(
                |(((name, args), ret), body): (
                    ((String, Vec<crate::SSAValue<'tokens, 'src, SimpleLang>>), crate::TypeofSSAValue<'tokens, 'src, SimpleLang>),
                    Option<crate::Region<'tokens, 'src, SimpleLang>>,
                )| {
                let args = args
                    .into_iter()
                    .map(|ssa| SimpleLang::to_ssa(ssa.name))
                    .collect();
                let body_vec = body
                    .unwrap_or_else(|| crate::Region { blocks: vec![] })
                    .blocks
                    .into_iter()
                    .flat_map(|block| block.statements.into_iter())
                    .collect();
                SimpleLang::Function {
                    name,
                    args,
                    ret: ret.ty,
                    body: body_vec,
                }
            },
            );

        add_parser.or(fn_parser).boxed()
    }
}

#[test]
fn parses_simple_language_statement() {
    let src = "add %lhs %rhs -> f64";
    let token_iter = kirin_lexer::Token::lexer(src)
        .spanned()
        .map(|(tok, span)| (tok.expect("lex"), span.into()));
    let token_stream =
        Stream::from_iter(token_iter).map((0..src.len()).into(), |(t, s): (_, _)| (t, s));

    let parser = SimpleLang::parser();
    let parsed = parser.parse(token_stream).into_result().expect("parse");

    match parsed {
        SimpleLang::Add {
            lhs,
            rhs,
            result_ty,
            ..
        } => {
            assert_eq!(result_ty, SimpleTy::F64);
            assert_eq!(lhs, SimpleLang::to_ssa("lhs"));
            assert_eq!(rhs, SimpleLang::to_ssa("rhs"));
        }
        other => panic!("expected add, got {:?}", other),
    }
}

#[test]
fn parses_function_with_region() {
    let src = "fn main(%arg0: f64) -> f64 ^entry() { add %lhs %rhs -> f64 }";
    let token_iter = kirin_lexer::Token::lexer(src)
        .spanned()
        .map(|(tok, span)| (tok.expect("lex"), span.into()));
    let token_stream =
        Stream::from_iter(token_iter).map((0..src.len()).into(), |(t, s): (_, _)| (t, s));

    let parser = SimpleLang::parser();
    let parsed = parser.parse(token_stream).into_result().expect("parse");

    match parsed {
        SimpleLang::Function { name, args, ret, body } => {
            assert_eq!(name, "main");
            assert_eq!(ret, SimpleTy::F64);
            assert_eq!(args.len(), 1);
            assert_eq!(body.len(), 1);
            assert!(matches!(body[0], SimpleLang::Add { .. }));
        }
        other => panic!("expected function, got {:?}", other),
    }
}
