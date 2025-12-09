use super::*;
use chumsky::{input::Stream, prelude::*};
use kirin_test_utils::*;
use logos::Logos;

#[derive(Debug, Clone)]
pub enum SimpleAST<'tokens, 'src: 'tokens> {
    Add {
        lhs: &'src str,
        rhs: &'src str,
        result: &'src str,
    },
    Constant {
        value: Value,
        result: &'src str,
    },
    Return(&'src str),
    Function {
        name: &'src str,
        input_types: Vec<SimpleTypeLattice>,
        output_type: SimpleTypeLattice,
        body: ast::Block<'tokens, 'src, SimpleLanguage>,
        result: &'src str,
    },
}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for SimpleTypeLattice
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

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for SimpleLanguage
where
    'src: 'tokens,
{
    type Output = SimpleAST<'tokens, 'src>;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        recursive(|dialect| {
            let ssa = just(Token::Percent).ignore_then(select! { Token::Identifier(name) => name });
            let operand_2 = ssa
                .clone()
                .then_ignore(just(Token::Comma))
                .then(ssa.clone())
                .labelled("2 operands");
            let input_types = SimpleTypeLattice::parser::<I>()
                .separated_by(just(Token::Comma))
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or(empty().to(Vec::new()))
                .labelled("function input types");
            let output_type = just(Token::Arrow)
                .ignore_then(SimpleTypeLattice::parser::<I>())
                .labelled("function output type");
            choice((
                // %result = add %lhs, %rhs
                ssa.clone()
                    .then_ignore(just(Token::Equal))
                    .then_ignore(just(Token::Identifier("add")))
                    .then(operand_2)
                    .map(|(result, (lhs, rhs))| SimpleAST::Add { lhs, rhs, result })
                    .labelled("addition instruction with result"),
                // Parse a constant instruction
                ssa.clone()
                    .then_ignore(just(Token::Equal))
                    .then_ignore(just(Token::Identifier("constant")))
                    .then(select! {
                        Token::Integer(v) => Value::I64(v),
                        Token::Float(v) => Value::F64(v),
                    })
                    .map(|(result, value)| SimpleAST::Constant { value, result })
                    .labelled("constant instruction with result"),
                // Parse a return instruction
                just(Token::Return)
                    .ignore_then(ssa.clone())
                    .map(|result| SimpleAST::Return(result))
                    .labelled("return instruction"),
                // Parse a function definition
                ssa.then_ignore(just(Token::Equal))
                    .then_ignore(just(Token::Fn))
                    .then_ignore(just(Token::At))
                    .then(select! { Token::Identifier(name) => name })
                    .then(input_types)
                    .then(output_type)
                    .then(
                        block_parser(dialect)
                            .delimited_by(just(Token::LBrace), just(Token::RBrace)),
                    )
                    .map(|((((result, name), input_types), output_type), body)| {
                        SimpleAST::Function {
                            name,
                            input_types,
                            output_type,
                            body,
                            result,
                        }
                    }),
            ))
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
                eprintln!("Parsing error: {:?}", error);
            }
            panic!("Failed to parse the input source.");
        }
    }
}
