//! Integration tests for kirin-chumsky-derive macros.

use chumsky::input::Stream;
use chumsky::prelude::*;
use kirin::ir::{Dialect, FiniteLattice, Lattice, ResultValue, SSAValue, TypeLattice};
use kirin_chumsky::{BoxedParser, HasParser, TokenInput};
use kirin_chumsky_derive::{HasRecursiveParser as DeriveRecursiveParser, WithAbstractSyntaxTree};
use kirin_lexer::{Logos, Token};

// === Simple Type Lattice ===

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimpleType {
    I32,
    I64,
    F32,
    F64,
    Bool,
    Unit,
}

impl Lattice for SimpleType {
    fn join(&self, other: &Self) -> Self {
        if self == other {
            self.clone()
        } else {
            SimpleType::Unit
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self == other {
            self.clone()
        } else {
            SimpleType::Unit
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        self == other || matches!(other, SimpleType::Unit)
    }
}

impl FiniteLattice for SimpleType {
    fn bottom() -> Self {
        SimpleType::Unit
    }

    fn top() -> Self {
        SimpleType::Unit
    }
}

impl TypeLattice for SimpleType {}

// Implement HasParser for SimpleType
impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for SimpleType {
    type Output = SimpleType;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        select! {
            Token::Identifier("i32") => SimpleType::I32,
            Token::Identifier("i64") => SimpleType::I64,
            Token::Identifier("f32") => SimpleType::F32,
            Token::Identifier("f64") => SimpleType::F64,
            Token::Identifier("bool") => SimpleType::Bool,
            Token::Identifier("unit") => SimpleType::Unit,
        }
        .labelled("type")
        .boxed()
    }
}

// === Test Language ===
// This single type implements both Dialect (via derive) and HasRecursiveParser (via derive)
// The blanket impl of HasParser for LanguageParser kicks in automatically!
//
// Format strings follow the new syntax (from AGENT.md):
// - `{field:name}` parses the SSA value name (e.g., `%result`)
// - `{field:type}` parses the type annotation (e.g., `i32`)
// - The same field can appear multiple times with different options

#[derive(Debug, Clone, PartialEq, Dialect, DeriveRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum TestLang {
    // Syntax: `%result = add %a %b -> i32`
    // Format: `{res:name} = add {lhs} {rhs} -> {res:type}`
    #[chumsky(format = "{res:name} = add {lhs} {rhs} -> {res:type}")]
    Add {
        res: ResultValue,
        lhs: SSAValue,
        rhs: SSAValue,
    },
    // Syntax: `%result = mul %a: i32, %b -> i64`
    // Format shows lhs with explicit name and type: `{lhs:name}: {lhs:type}`
    #[chumsky(format = "{res:name} = mul {lhs:name}: {lhs:type}, {rhs} -> {res:type}")]
    Mul {
        res: ResultValue,
        lhs: SSAValue,
        rhs: SSAValue,
    },
    // Syntax: `return %value` (no result assignment)
    #[chumsky(format = "return {0}")]
    Return(SSAValue),
}

// === Helper function to parse ===

fn parse_input(input: &str) -> Result<TestLangAST<'_, '_, TestLang>, Vec<String>> {
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, chumsky::span::SimpleSpan::from(span))
        })
        .collect();

    let stream = Stream::from_iter(tokens).map((0..input.len()).into(), |(t, s): (_, _)| (t, s));

    // TestLang implements LanguageParser (because it implements Dialect + HasRecursiveParser)
    // which gives it a blanket impl of HasParser - so we can just call parser() directly!
    let parser = <TestLang as HasParser<'_, '_>>::parser();
    let result = parser.parse(stream);

    if result.has_output() {
        Ok(result.into_output().unwrap())
    } else {
        Err(result.errors().map(|e| format!("{:?}", e)).collect())
    }
}

// === Tests ===

#[test]
fn test_parse_add() {
    // Syntax: `%result = add %a %b -> i32`
    let result = parse_input("%result = add %a %b -> i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        TestLangAST::Add { res, lhs, rhs } => {
            // res has both name (from {res:name}) and type (from {res:type})
            assert_eq!(res.name.value, "result");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(lhs.name.value, "a");
            assert_eq!(rhs.name.value, "b");
        }
        _ => panic!("Expected Add variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_mul() {
    // Syntax: `%x = mul %y: i32, %z -> i64`
    let result = parse_input("%x = mul %y: i32, %z -> i64");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        TestLangAST::Mul { res, lhs, rhs } => {
            assert_eq!(res.name.value, "x");
            assert_eq!(res.ty, Some(SimpleType::I64));
            // lhs has both name and type from separate format elements
            assert_eq!(lhs.name.value, "y");
            assert_eq!(lhs.ty, Some(SimpleType::I32));
            assert_eq!(rhs.name.value, "z");
        }
        _ => panic!("Expected Mul variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_return() {
    // Return has no result assignment
    let result = parse_input("return %value");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        TestLangAST::Return(ssa) => {
            assert_eq!(ssa.name.value, "value");
        }
        _ => panic!("Expected Return variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_fails_on_invalid_input() {
    let result = parse_input("invalid syntax here");
    assert!(result.is_err(), "Expected parse to fail");
}

#[test]
fn test_parse_multiple_operations() {
    // Test parsing a second add operation with different values
    let result = parse_input("%output = add %x %y -> f32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        TestLangAST::Add { res, lhs, rhs } => {
            assert_eq!(res.name.value, "output");
            assert_eq!(res.ty, Some(SimpleType::F32));
            assert_eq!(lhs.name.value, "x");
            assert_eq!(rhs.name.value, "y");
        }
        _ => panic!("Expected Add variant, got {:?}", ast),
    }
}

// ============================================================================
// Tests for Block and Region fields
// ============================================================================

/// A language that includes statements with Block and Region fields.
/// This tests the recursive parsing capability of the derive macros.
#[derive(Debug, Clone, PartialEq, Dialect, DeriveRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum BlockRegionLang {
    // Simple value statement (needed for block body content)
    // Syntax: `%result = id %value -> i32`
    #[chumsky(format = "{res:name} = id {arg} -> {res:type}")]
    Id { res: ResultValue, arg: SSAValue },

    // Statement with a single Block field
    // Syntax: `loop ^entry(%i: i32) { %x = id %i -> i32; } -> i64`
    // The block parses: ^label(args) { statements }
    #[chumsky(format = "loop {body} -> {res:type}")]
    Loop {
        res: ResultValue,
        body: kirin::ir::Block,
    },

    // Statement with a Region field (contains multiple blocks)
    // Syntax: `scope { ^bb0() { ... }; ^bb1() { ... }; } -> bool`
    #[chumsky(format = "scope {body} -> {res:type}")]
    Scope {
        res: ResultValue,
        body: kirin::ir::Region,
    },

    // Return statement (terminator)
    #[chumsky(format = "ret {0}")]
    Ret(SSAValue),
}

/// Helper function to parse BlockRegionLang input
fn parse_block_region_input(
    input: &str,
) -> Result<BlockRegionLangAST<'_, '_, BlockRegionLang>, Vec<String>> {
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, chumsky::span::SimpleSpan::from(span))
        })
        .collect();

    let stream = Stream::from_iter(tokens).map((0..input.len()).into(), |(t, s): (_, _)| (t, s));

    let parser = <BlockRegionLang as HasParser<'_, '_>>::parser();
    let result = parser.parse(stream);

    if result.has_output() {
        Ok(result.into_output().unwrap())
    } else {
        Err(result.errors().map(|e| format!("{:?}", e)).collect())
    }
}

// === Block Tests ===

#[test]
fn test_parse_block_empty_body() {
    // A loop with an empty block (no statements inside)
    // Block syntax: ^label(args) { }
    let result = parse_block_region_input("loop ^entry() { } -> i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::I32));
            // Check block header (body is Spanned<Block>, so access .value first)
            assert_eq!(body.value.header.value.label.name.value, "entry");
            assert!(body.value.header.value.arguments.is_empty());
            // Check empty body
            assert!(body.value.statements.is_empty());
        }
        _ => panic!("Expected Loop variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_block_with_arguments() {
    // A loop with block arguments
    let result = parse_block_region_input("loop ^bb0(%x: i32, %y: f64) { } -> bool");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::Bool));
            assert_eq!(body.value.header.value.label.name.value, "bb0");
            // Check arguments
            assert_eq!(body.value.header.value.arguments.len(), 2);
            assert_eq!(body.value.header.value.arguments[0].value.name.value, "x");
            assert_eq!(
                body.value.header.value.arguments[0].value.ty.value,
                SimpleType::I32
            );
            assert_eq!(body.value.header.value.arguments[1].value.name.value, "y");
            assert_eq!(
                body.value.header.value.arguments[1].value.ty.value,
                SimpleType::F64
            );
        }
        _ => panic!("Expected Loop variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_block_with_statements() {
    // A loop with statements inside the block
    let result = parse_block_region_input("loop ^body(%n: i32) { %r = id %n -> i32; } -> i64");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::I64));
            assert_eq!(body.value.header.value.label.name.value, "body");
            assert_eq!(body.value.header.value.arguments.len(), 1);
            // Check that we have one statement
            assert_eq!(body.value.statements.len(), 1);
            // Verify the statement content
            match &body.value.statements[0].value {
                BlockRegionLangAST::Id { res, arg } => {
                    assert_eq!(res.name.value, "r");
                    assert_eq!(res.ty, Some(SimpleType::I32));
                    assert_eq!(arg.name.value, "n");
                }
                _ => panic!("Expected Id statement inside block"),
            }
        }
        _ => panic!("Expected Loop variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_block_with_multiple_statements() {
    // A block with multiple statements
    let result = parse_block_region_input(
        "loop ^main(%a: i32) { %b = id %a -> i32; %c = id %b -> f32; } -> unit",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::Unit));
            assert_eq!(body.value.statements.len(), 2);

            // First statement
            match &body.value.statements[0].value {
                BlockRegionLangAST::Id { res, arg } => {
                    assert_eq!(res.name.value, "b");
                    assert_eq!(arg.name.value, "a");
                }
                _ => panic!("Expected first Id statement"),
            }

            // Second statement
            match &body.value.statements[1].value {
                BlockRegionLangAST::Id { res, arg } => {
                    assert_eq!(res.name.value, "c");
                    assert_eq!(arg.name.value, "b");
                }
                _ => panic!("Expected second Id statement"),
            }
        }
        _ => panic!("Expected Loop variant, got {:?}", ast),
    }
}

// === Region Tests ===

#[test]
fn test_parse_region_empty() {
    // A scope with an empty region (no blocks)
    let result = parse_block_region_input("scope { } -> i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert!(body.blocks.is_empty());
        }
        _ => panic!("Expected Scope variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_region_single_block() {
    // A scope with one block inside the region
    let result = parse_block_region_input("scope { ^entry() { }; } -> f32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::F32));
            assert_eq!(body.blocks.len(), 1);
            assert_eq!(body.blocks[0].value.header.value.label.name.value, "entry");
        }
        _ => panic!("Expected Scope variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_region_multiple_blocks() {
    // A scope with multiple blocks
    let result =
        parse_block_region_input("scope { ^bb0(%x: i32) { }; ^bb1() { }; ^exit() { }; } -> bool");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::Bool));
            assert_eq!(body.blocks.len(), 3);

            // First block
            assert_eq!(body.blocks[0].value.header.value.label.name.value, "bb0");
            assert_eq!(body.blocks[0].value.header.value.arguments.len(), 1);
            assert_eq!(
                body.blocks[0].value.header.value.arguments[0]
                    .value
                    .name
                    .value,
                "x"
            );

            // Second block
            assert_eq!(body.blocks[1].value.header.value.label.name.value, "bb1");
            assert!(body.blocks[1].value.header.value.arguments.is_empty());

            // Third block
            assert_eq!(body.blocks[2].value.header.value.label.name.value, "exit");
        }
        _ => panic!("Expected Scope variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_region_with_statements_in_blocks() {
    // A region with blocks containing statements
    let result = parse_block_region_input(
        "scope { ^bb0(%a: i32) { %b = id %a -> i64; }; ^bb1() { %c = id %b -> f32; }; } -> unit",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::Unit));
            assert_eq!(body.blocks.len(), 2);

            // First block has one statement
            assert_eq!(body.blocks[0].value.statements.len(), 1);
            match &body.blocks[0].value.statements[0].value {
                BlockRegionLangAST::Id { res, arg } => {
                    assert_eq!(res.name.value, "b");
                    assert_eq!(arg.name.value, "a");
                }
                _ => panic!("Expected Id in first block"),
            }

            // Second block has one statement
            assert_eq!(body.blocks[1].value.statements.len(), 1);
            match &body.blocks[1].value.statements[0].value {
                BlockRegionLangAST::Id { res, arg } => {
                    assert_eq!(res.name.value, "c");
                    assert_eq!(arg.name.value, "b");
                }
                _ => panic!("Expected Id in second block"),
            }
        }
        _ => panic!("Expected Scope variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_region_without_trailing_semicolon() {
    // Region blocks can optionally omit trailing semicolon on last block
    let result = parse_block_region_input("scope { ^only() { } } -> i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(body.blocks.len(), 1);
            assert_eq!(body.blocks[0].value.header.value.label.name.value, "only");
        }
        _ => panic!("Expected Scope variant, got {:?}", ast),
    }
}

// === Error Cases ===

#[test]
fn test_parse_block_missing_label() {
    // Block requires a label like ^name
    let result = parse_block_region_input("loop () { } -> i32");
    assert!(
        result.is_err(),
        "Expected parse to fail for missing block label"
    );
}

#[test]
fn test_parse_block_missing_braces() {
    // Block requires { } around statements
    let result = parse_block_region_input("loop ^bb0() -> i32");
    assert!(result.is_err(), "Expected parse to fail for missing braces");
}

