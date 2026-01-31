//! Integration tests for kirin-chumsky-derive macros.

use chumsky::input::Stream;
use chumsky::prelude::*;
use kirin::ir::{Dialect, FiniteLattice, Lattice, ResultValue, SSAValue, Successor, TypeLattice};
use kirin_chumsky::{BoxedParser, HasParser, TokenInput};
use kirin_chumsky_derive::{
    DialectParser, HasRecursiveParser as DeriveRecursiveParser, WithAbstractSyntaxTree,
};
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
    // Syntax: `%res = loop ^entry(%i: i32) { %x = id %i -> i32; }`
    // The block parses: ^label(args) { statements }
    #[chumsky(format = "{res} = loop {body}")]
    Loop {
        res: ResultValue,
        body: kirin::ir::Block,
    },

    // Statement with a Region field (contains multiple blocks)
    // Syntax: `%res = scope { ^bb0() { ... }; ^bb1() { ... }; }`
    #[chumsky(format = "{res} = scope {body}")]
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
    let result = parse_block_region_input("%out = loop ^entry() { }");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, None);
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
    let result = parse_block_region_input("%res: bool = loop ^bb0(%x: i32, %y: f64) { }");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "res");
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
    let result = parse_block_region_input("%res: i64 = loop ^body(%n: i32) { %r = id %n -> i32; }");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "res");
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
        "%res: unit = loop ^main(%a: i32) { %b = id %a -> i32; %c = id %b -> f32; }",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "res");
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
    let result = parse_block_region_input("%out: i32 = scope { }");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert!(body.blocks.is_empty());
        }
        _ => panic!("Expected Scope variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_region_single_block() {
    // A scope with one block inside the region
    let result = parse_block_region_input("%out: f32 = scope { ^entry() { }; }");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
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
    let result = parse_block_region_input(
        "%out: bool = scope { ^bb0(%x: i32) { }; ^bb1() { }; ^exit() { }; }",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
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
        "%out: unit = scope { ^bb0(%a: i32) { %b = id %a -> i64; }; ^bb1() { %c = id %b -> f32; }; }",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
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
    let result = parse_block_region_input("%out: i32 = scope { ^only() { } }");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
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
    let result = parse_block_region_input("%out = loop () { }");
    assert!(
        result.is_err(),
        "Expected parse to fail for missing block label"
    );
}

#[test]
fn test_parse_block_missing_braces() {
    // Block requires { } around statements
    let result = parse_block_region_input("%out = loop ^bb0()");
    assert!(result.is_err(), "Expected parse to fail for missing braces");
}

// ============================================================================
// Tests for Successor fields
// ============================================================================

/// A language that includes control flow operations with Successor fields.
#[derive(Debug, Clone, PartialEq, Dialect, DeriveRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum ControlFlowLang {
    #[chumsky(format = "{res:name} = id {arg} -> {res:type}")]
    Id { res: ResultValue, arg: SSAValue },
    #[chumsky(format = "br {target}")]
    Branch { target: Successor },
    #[chumsky(format = "cond_br {cond} then = {true_target} else = {false_target}")]
    CondBranch {
        cond: SSAValue,
        true_target: Successor,
        false_target: Successor,
    },
}

fn parse_control_flow_input(
    input: &str,
) -> Result<ControlFlowLangAST<'_, '_, ControlFlowLang>, Vec<String>> {
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, chumsky::span::SimpleSpan::from(span))
        })
        .collect();

    let stream = Stream::from_iter(tokens).map((0..input.len()).into(), |(t, s): (_, _)| (t, s));
    let parser = <ControlFlowLang as HasParser<'_, '_>>::parser();
    let result = parser.parse(stream);

    if result.has_output() {
        Ok(result.into_output().unwrap())
    } else {
        Err(result.errors().map(|e| format!("{:?}", e)).collect())
    }
}

#[test]
fn test_parse_successor_branch() {
    let result = parse_control_flow_input("br ^exit");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        ControlFlowLangAST::Branch { target } => {
            assert_eq!(target.name.value, "exit");
        }
        _ => panic!("Expected Branch variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_successor_cond_branch() {
    let result = parse_control_flow_input("cond_br %flag then = ^bb1 else = ^bb2");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        ControlFlowLangAST::CondBranch {
            cond,
            true_target,
            false_target,
        } => {
            assert_eq!(cond.name.value, "flag");
            assert_eq!(true_target.name.value, "bb1");
            assert_eq!(false_target.name.value, "bb2");
        }
        _ => panic!("Expected CondBranch variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_successor_numeric_label() {
    let result = parse_control_flow_input("br ^0");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        ControlFlowLangAST::Branch { target } => {
            assert_eq!(target.name.value, "0");
        }
        _ => panic!("Expected Branch variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_successor_missing_caret() {
    // Successor requires a block label starting with ^
    let result = parse_control_flow_input("br exit");
    assert!(
        result.is_err(),
        "Expected parse to fail for missing ^ in block label"
    );
}

// ============================================================================
// Tests for struct-based dialects
// ============================================================================

// Note: Struct-based dialects are not tested here because the Dialect derive
// macro for structs requires additional trait implementations that aren't
// auto-generated the same way as for enums. This is a known limitation.

// ============================================================================
// Tests for DialectParser combined derive
// ============================================================================

/// A dialect using the combined DialectParser derive macro.
#[derive(Debug, Clone, PartialEq, Dialect, DialectParser)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum CombinedLang {
    #[chumsky(format = "{res:name} = inc {arg} -> {res:type}")]
    Inc { res: ResultValue, arg: SSAValue },
    #[chumsky(format = "{res:name} = dec {arg} -> {res:type}")]
    Dec { res: ResultValue, arg: SSAValue },
}

fn parse_combined_lang_input(
    input: &str,
) -> Result<CombinedLangAST<'_, '_, CombinedLang>, Vec<String>> {
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, chumsky::span::SimpleSpan::from(span))
        })
        .collect();

    let stream = Stream::from_iter(tokens).map((0..input.len()).into(), |(t, s): (_, _)| (t, s));
    let parser = <CombinedLang as HasParser<'_, '_>>::parser();
    let result = parser.parse(stream);

    if result.has_output() {
        Ok(result.into_output().unwrap())
    } else {
        Err(result.errors().map(|e| format!("{:?}", e)).collect())
    }
}

#[test]
fn test_parse_combined_derive_inc() {
    let result = parse_combined_lang_input("%r = inc %x -> i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        CombinedLangAST::Inc { res, arg } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(arg.name.value, "x");
        }
        _ => panic!("Expected Inc variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_combined_derive_dec() {
    // Second variant using combined DialectParser derive
    let result = parse_combined_lang_input("%y = dec %z -> f64");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        CombinedLangAST::Dec { res, arg } => {
            assert_eq!(res.name.value, "y");
            assert_eq!(res.ty, Some(SimpleType::F64));
            assert_eq!(arg.name.value, "z");
        }
        _ => panic!("Expected Dec variant, got {:?}", ast),
    }
}

// ============================================================================
// Tests for SSAValue default format with inline type
// ============================================================================

#[test]
fn test_parse_ssa_default_with_type() {
    // SSAValue default format parses optional type: %x: i32
    let result = parse_input("return %x: i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        TestLangAST::Return(ssa) => {
            assert_eq!(ssa.name.value, "x");
            assert_eq!(ssa.ty, Some(SimpleType::I32));
        }
        _ => panic!("Expected Return variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_ssa_default_without_type() {
    // SSAValue default format with no type annotation
    let result = parse_input("return %x");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        TestLangAST::Return(ssa) => {
            assert_eq!(ssa.name.value, "x");
            assert!(ssa.ty.is_none());
        }
        _ => panic!("Expected Return variant, got {:?}", ast),
    }
}

// ============================================================================
// Tests for multiple positional tuple fields
// ============================================================================

/// A dialect with tuple variants using multiple positional fields.
#[derive(Debug, Clone, PartialEq, Dialect, DeriveRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum TupleLang {
    #[chumsky(format = "swap {0} {1}")]
    Swap(SSAValue, SSAValue),
    // Use named fields for more complex operations to avoid tuple ordering issues
    #[chumsky(format = "{res:name} = sel {cond} {left} {right} -> {res:type}")]
    Select {
        res: ResultValue,
        cond: SSAValue,
        left: SSAValue,
        right: SSAValue,
    },
}

fn parse_tuple_lang_input(input: &str) -> Result<TupleLangAST<'_, '_, TupleLang>, Vec<String>> {
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, chumsky::span::SimpleSpan::from(span))
        })
        .collect();

    let stream = Stream::from_iter(tokens).map((0..input.len()).into(), |(t, s): (_, _)| (t, s));
    let parser = <TupleLang as HasParser<'_, '_>>::parser();
    let result = parser.parse(stream);

    if result.has_output() {
        Ok(result.into_output().unwrap())
    } else {
        Err(result.errors().map(|e| format!("{:?}", e)).collect())
    }
}

#[test]
fn test_parse_tuple_two_positional() {
    let result = parse_tuple_lang_input("swap %a %b");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        TupleLangAST::Swap(first, second) => {
            assert_eq!(first.name.value, "a");
            assert_eq!(second.name.value, "b");
        }
        _ => panic!("Expected Swap variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_named_fields_four_fields() {
    let result = parse_tuple_lang_input("%out = sel %cond %left %right -> i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        TupleLangAST::Select {
            res,
            cond,
            left,
            right,
        } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(cond.name.value, "cond");
            assert_eq!(left.name.value, "left");
            assert_eq!(right.name.value, "right");
        }
        _ => panic!("Expected Select variant, got {:?}", ast),
    }
}

// ============================================================================
// Tests for ResultValue :name only (no :type)
// ============================================================================

/// A dialect where some operations don't have a result type in the syntax.
#[derive(Debug, Clone, PartialEq, Dialect, DeriveRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum UnaryLang {
    // Result type not in syntax - inferred later
    #[chumsky(format = "{res:name} = neg {arg}")]
    Neg { res: ResultValue, arg: SSAValue },
    // Result type explicitly in syntax
    #[chumsky(format = "{res:name} = abs {arg} -> {res:type}")]
    Abs { res: ResultValue, arg: SSAValue },
}

fn parse_unary_lang_input(input: &str) -> Result<UnaryLangAST<'_, '_, UnaryLang>, Vec<String>> {
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, chumsky::span::SimpleSpan::from(span))
        })
        .collect();

    let stream = Stream::from_iter(tokens).map((0..input.len()).into(), |(t, s): (_, _)| (t, s));
    let parser = <UnaryLang as HasParser<'_, '_>>::parser();
    let result = parser.parse(stream);

    if result.has_output() {
        Ok(result.into_output().unwrap())
    } else {
        Err(result.errors().map(|e| format!("{:?}", e)).collect())
    }
}

#[test]
fn test_parse_result_name_only() {
    let result = parse_unary_lang_input("%x = neg %y");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        UnaryLangAST::Neg { res, arg } => {
            assert_eq!(res.name.value, "x");
            assert!(res.ty.is_none(), "Expected ty to be None for :name only");
            assert_eq!(arg.name.value, "y");
        }
        _ => panic!("Expected Neg variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_result_name_and_type() {
    let result = parse_unary_lang_input("%x = abs %y -> i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        UnaryLangAST::Abs { res, arg } => {
            assert_eq!(res.name.value, "x");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(arg.name.value, "y");
        }
        _ => panic!("Expected Abs variant, got {:?}", ast),
    }
}

// ============================================================================
// Tests for compile-time value fields (non-IR types with HasParser)
// ============================================================================

/// A custom compile-time value type that parses any identifier.
#[derive(Debug, Clone, PartialEq)]
pub struct Opcode(pub String);

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for Opcode {
    type Output = Opcode;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        select! {
            Token::Identifier(name) => Opcode(name.to_string())
        }
        .labelled("opcode")
        .boxed()
    }
}

/// A dialect that uses a compile-time value field.
#[derive(Debug, Clone, PartialEq, Dialect, DeriveRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum ValueLang {
    #[chumsky(format = "{res:name} = apply {op} {arg} -> {res:type}")]
    Apply {
        res: ResultValue,
        op: Opcode,
        arg: SSAValue,
    },
}

fn parse_value_lang_input(input: &str) -> Result<ValueLangAST<'_, '_, ValueLang>, Vec<String>> {
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, chumsky::span::SimpleSpan::from(span))
        })
        .collect();

    let stream = Stream::from_iter(tokens).map((0..input.len()).into(), |(t, s): (_, _)| (t, s));
    let parser = <ValueLang as HasParser<'_, '_>>::parser();
    let result = parser.parse(stream);

    if result.has_output() {
        Ok(result.into_output().unwrap())
    } else {
        Err(result.errors().map(|e| format!("{:?}", e)).collect())
    }
}

#[test]
fn test_parse_compile_time_value() {
    let result = parse_value_lang_input("%r = apply custom_op %x -> i32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        ValueLangAST::Apply { res, op, arg } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(op, Opcode("custom_op".to_string()));
            assert_eq!(arg.name.value, "x");
        }
    }
}

#[test]
fn test_parse_compile_time_value_different() {
    let result = parse_value_lang_input("%r = apply another %x -> f32");
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        ValueLangAST::Apply { res, op, arg } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(res.ty, Some(SimpleType::F32));
            assert_eq!(op, Opcode("another".to_string()));
            assert_eq!(arg.name.value, "x");
        }
    }
}

// ============================================================================
// Tests for deep recursive nesting
// ============================================================================

#[test]
fn test_parse_nested_loop_in_scope() {
    // A scope containing a block with a loop statement inside
    let result = parse_block_region_input(
        "%out: unit = scope { ^bb0() { %inner_res: i32 = loop ^inner() { }; }; }",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::Unit));
            assert_eq!(body.blocks.len(), 1);
            assert_eq!(body.blocks[0].value.header.value.label.name.value, "bb0");

            // Check the nested loop statement
            assert_eq!(body.blocks[0].value.statements.len(), 1);
            match &body.blocks[0].value.statements[0].value {
                BlockRegionLangAST::Loop { res, body } => {
                    assert_eq!(res.name.value, "inner_res");
                    assert_eq!(res.ty, Some(SimpleType::I32));
                    assert_eq!(body.value.header.value.label.name.value, "inner");
                }
                _ => panic!("Expected Loop statement inside block"),
            }
        }
        _ => panic!("Expected Scope variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_nested_scope_in_loop() {
    // A loop containing a scope statement inside its block
    let result = parse_block_region_input(
        "%out: unit = loop ^outer() { %inner_res: i32 = scope { ^inner() { } }; }",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::Unit));
            assert_eq!(body.value.header.value.label.name.value, "outer");

            // Check the nested scope statement
            assert_eq!(body.value.statements.len(), 1);
            match &body.value.statements[0].value {
                BlockRegionLangAST::Scope { res, body } => {
                    assert_eq!(res.name.value, "inner_res");
                    assert_eq!(res.ty, Some(SimpleType::I32));
                    assert_eq!(body.blocks.len(), 1);
                    assert_eq!(body.blocks[0].value.header.value.label.name.value, "inner");
                }
                _ => panic!("Expected Scope statement inside loop"),
            }
        }
        _ => panic!("Expected Loop variant, got {:?}", ast),
    }
}

#[test]
fn test_parse_deeply_nested_structure() {
    // scope -> block -> loop -> block -> scope -> block
    let result = parse_block_region_input(
        "%out: unit = scope { ^bb0() { %loop_res: i64 = loop ^loop0() { %scope_res: bool = scope { ^bb1() { } }; }; }; }",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Scope { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::Unit));

            // First level: bb0
            let bb0 = &body.blocks[0].value;
            assert_eq!(bb0.header.value.label.name.value, "bb0");
            assert_eq!(bb0.statements.len(), 1);

            // Second level: loop -> loop0
            match &bb0.statements[0].value {
                BlockRegionLangAST::Loop { res, body } => {
                    assert_eq!(res.name.value, "loop_res");
                    assert_eq!(res.ty, Some(SimpleType::I64));
                    assert_eq!(body.value.header.value.label.name.value, "loop0");
                    assert_eq!(body.value.statements.len(), 1);

                    // Third level: scope -> bb1
                    match &body.value.statements[0].value {
                        BlockRegionLangAST::Scope { res, body } => {
                            assert_eq!(res.name.value, "scope_res");
                            assert_eq!(res.ty, Some(SimpleType::Bool));
                            assert_eq!(body.blocks.len(), 1);
                            assert_eq!(body.blocks[0].value.header.value.label.name.value, "bb1");
                        }
                        _ => panic!("Expected nested Scope"),
                    }
                }
                _ => panic!("Expected Loop at second level"),
            }
        }
        _ => panic!("Expected Scope variant, got {:?}", ast),
    }
}

// ============================================================================
// Additional edge case tests
// ============================================================================

#[test]
fn test_parse_block_argument_with_all_types() {
    // Test block arguments with various type lattice values
    let result = parse_block_region_input(
        "%out: i32 = loop ^bb0(%a: i32, %b: i64, %c: f32, %d: f64, %e: bool, %f: unit) { }",
    );
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

    let ast = result.unwrap();
    match ast {
        BlockRegionLangAST::Loop { res, body } => {
            assert_eq!(res.name.value, "out");
            assert_eq!(res.ty, Some(SimpleType::I32));
            let args = &body.value.header.value.arguments;
            assert_eq!(args.len(), 6);
            assert_eq!(args[0].value.ty.value, SimpleType::I32);
            assert_eq!(args[1].value.ty.value, SimpleType::I64);
            assert_eq!(args[2].value.ty.value, SimpleType::F32);
            assert_eq!(args[3].value.ty.value, SimpleType::F64);
            assert_eq!(args[4].value.ty.value, SimpleType::Bool);
            assert_eq!(args[5].value.ty.value, SimpleType::Unit);
        }
        _ => panic!("Expected Loop variant"),
    }
}

#[test]
fn test_parse_multiple_variants_same_dialect() {
    // Ensure multiple variants work correctly in the same dialect
    // First try parsing Inc variant
    let result1 = parse_combined_lang_input("%r = inc %x -> i32");
    assert!(result1.is_ok(), "Failed to parse Inc: {:?}", result1.err());
    match result1.unwrap() {
        CombinedLangAST::Inc { res, arg } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(arg.name.value, "x");
        }
        _ => panic!("Expected Inc variant"),
    }

    // Then parse Dec variant
    let result2 = parse_combined_lang_input("%y = dec %z -> f64");
    assert!(result2.is_ok(), "Failed to parse Dec: {:?}", result2.err());
    match result2.unwrap() {
        CombinedLangAST::Dec { res, arg } => {
            assert_eq!(res.name.value, "y");
            assert_eq!(arg.name.value, "z");
        }
        _ => panic!("Expected Dec variant"),
    }
}
