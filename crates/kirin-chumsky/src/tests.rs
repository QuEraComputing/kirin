use super::*;
use chumsky::{input::Stream, prelude::*};
use kirin_lexer::Token;
use kirin_test_utils::*;
use logos::Logos;

#[derive(Debug, Clone)]
pub enum SimpleAST<'tokens, 'src: 'tokens> {
    Add {
        lhs: ast::Spanned<&'src str>,
        rhs: ast::Spanned<&'src str>,
        result: ast::Spanned<&'src str>,
    },
    Constant {
        value: ast::Spanned<Value>,
        result: ast::Spanned<&'src str>,
    },
    Return(ast::Spanned<&'src str>),
    Function {
        name: ast::Spanned<&'src str>,
        function_type: ast::Spanned<ast::FunctionType<'tokens, 'src, SimpleLanguage>>,
        body: ast::Spanned<ast::Block<'tokens, 'src, SimpleLanguage>>,
        result: ast::Spanned<&'src str>,
    },
}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src, SimpleLanguage> for SimpleTypeLattice
where
    'src: 'tokens,
{
    type Output = SimpleTypeLattice;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        select! {
            Token::Identifier("i64") => SimpleTypeLattice::Int,
            Token::Identifier("f64") => SimpleTypeLattice::Float,
        }
        .boxed()
    }
}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src, SimpleLanguage> for SimpleLanguage
where
    'src: 'tokens,
{
    type Output = SimpleAST<'tokens, 'src>;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        recursive(|dialect| {
            let add = // %result = add %lhs, %rhs
                ssa_value()
                    .then_ignore(just(Token::Equal))
                    .then_ignore(identifier("add"))
                    .then(operand(2, Token::Comma))
                    .map(|(result, operands)| SimpleAST::Add { lhs: operands[0], rhs: operands[1], result })
                    .labelled("addition instruction with result");

            // Parse a constant instruction 
            let constant = ssa_value()
                    .then_ignore(just(Token::Equal))
                    .then_ignore(identifier("constant"))
                    .then(
                        literal_int(|i, span| match i.parse() {
                            Ok(v) => Ok(Value::I64(v)),
                            Err(_) => Err(Rich::custom(span, "invalid integer literal")),
                        })
                        .or(literal_float(|f, span| match f.parse() {
                            Ok(v) => Ok(Value::F64(v)),
                            Err(_) => Err(Rich::custom(span, "invalid float literal")),
                        }))
                    )
                    .map(|(result, value)| {
                        SimpleAST::Constant { value, result }
                    })
                    .labelled("constant instruction with result");

            let ret = identifier("return")
                    .ignore_then(ssa_value())
                    .map(|result| SimpleAST::Return(result))
                    .labelled("return instruction");

            let func = ssa_value()
                .then_ignore(just(Token::Equal))
                .then_ignore(identifier("fn"))
                .then(symbol())
                .then(function_type())
                .then(
                    block(dialect)
                        .delimited_by(just(Token::LBrace), just(Token::RBrace)),
                )
                .map(|(((result, name), function_type), body)| {
                    SimpleAST::Function {
                        name,
                        function_type,
                        body,
                        result,
                    }
                });

            choice((
                add,
                constant,
                ret,
                func,
            )).boxed()
        })
        .boxed()
    }
}

const SRC: &str = "
%f = fn @main(i64, f64) -> i64 {
    ^bb0(%x: i64, %y: f64) {
        %f1 = constant 1.2;
        %f2 = constant 3.4;
        %f3 = add %f1, %f2;
        %arg_x = add %f3, %x;
        return %arg_x;
    }
}
";

#[test]
fn test_simple_language_parser() {
    use ariadne::{Color, Label, Report, ReportKind, Source};

    let token_iter = Token::lexer(SRC).spanned().map(|(tok, span)| match tok {
        Ok(tok) => (tok, span.into()),
        Err(()) => (Token::Error, span.into()),
    });
    let token_stream =
        Stream::from_iter(token_iter).map((0..SRC.len()).into(), |(t, s): (_, _)| (t, s));

    // let parser = block_parser::<MappedInput<_, _, _, _>, SimpleLanguage>(SimpleLanguage::parser());
    // match parser.parse(token_stream).into_result() {
    match SimpleLanguage::parser().parse(token_stream).into_result() {
        Ok(ast) => {
            // Successfully parsed the AST
            println!("Parsed AST: {:?}", ast);
        }
        Err(errors) => {
            // Handle parsing errors
            for error in errors {
                Report::build(ReportKind::Error, ((), error.span().into_range()))
                    .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                    .with_code(3)
                    .with_message(error.to_string())
                    .with_label(
                        Label::new(((), error.span().into_range()))
                            .with_message(error.reason().to_string())
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint(Source::from(SRC))
                    .unwrap();
            }
        }
    }
}
