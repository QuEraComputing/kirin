//! Snapshot tests for code generation.
//!
//! These tests capture the current macro expansion behavior to ensure
//! consistency and detect unintended changes.

use crate::{
    ChumskyLayout, GenerateAST, GenerateEmitIR, GenerateHasDialectParser, GeneratePrettyPrint,
};
use kirin_derive_core::ir::Input;
use kirin_derive_core::test_util::rustfmt_tokens;

/// Helper to generate all outputs for a given input.
fn generate_all(input: &syn::DeriveInput) -> (String, String, String, String) {
    let ir_input: Input<ChumskyLayout> = Input::from_derive_input(input).unwrap();

    let ast_gen = GenerateAST::new(&ir_input);
    let emit_gen = GenerateEmitIR::new(&ir_input);
    let parser_gen = GenerateHasDialectParser::new(&ir_input);
    let pretty_gen = GeneratePrettyPrint::new(&ir_input);

    let ast = rustfmt_tokens(&ast_gen.generate(&ir_input));
    let emit = rustfmt_tokens(&emit_gen.generate(&ir_input));
    let parser = rustfmt_tokens(&parser_gen.generate(&ir_input));
    let pretty = rustfmt_tokens(&pretty_gen.generate(&ir_input));

    (ast, emit, parser, pretty)
}

// =============================================================================
// Simple Struct Tests
// =============================================================================

#[test]
fn test_struct_simple_add() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "{res:name} = add {lhs}, {rhs} -> {res:type}")]
        struct Add {
            lhs: SSAValue,
            rhs: SSAValue,
            #[kirin(type = SimpleType::Int)]
            res: ResultValue,
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("struct_simple_add_ast", ast);
    insta::assert_snapshot!("struct_simple_add_emit", emit);
    insta::assert_snapshot!("struct_simple_add_parser", parser);
    insta::assert_snapshot!("struct_simple_add_pretty", pretty);
}

#[test]
fn test_struct_tuple_style() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "return {0}")]
        struct Return(SSAValue);
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("struct_tuple_style_ast", ast);
    insta::assert_snapshot!("struct_tuple_style_emit", emit);
    insta::assert_snapshot!("struct_tuple_style_parser", parser);
    insta::assert_snapshot!("struct_tuple_style_pretty", pretty);
}

#[test]
fn test_struct_with_region() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "{res:name} = function {body} -> {res:type}")]
        struct Function {
            body: Region,
            #[kirin(type = SimpleType::Fn)]
            res: ResultValue,
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("struct_with_region_ast", ast);
    insta::assert_snapshot!("struct_with_region_emit", emit);
    insta::assert_snapshot!("struct_with_region_parser", parser);
    insta::assert_snapshot!("struct_with_region_pretty", pretty);
}

#[test]
fn test_struct_with_comptime_value() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "{res:name} = constant {value} -> {res:type}")]
        struct Constant {
            #[kirin(into)]
            value: Value,
            #[kirin(type = SimpleType::Int)]
            res: ResultValue,
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("struct_with_comptime_ast", ast);
    insta::assert_snapshot!("struct_with_comptime_emit", emit);
    insta::assert_snapshot!("struct_with_comptime_parser", parser);
    insta::assert_snapshot!("struct_with_comptime_pretty", pretty);
}

// =============================================================================
// Simple Enum Tests
// =============================================================================

#[test]
fn test_enum_simple() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        enum SimpleLang {
            #[chumsky(format = "{res:name} = add {lhs}, {rhs} -> {res:type}")]
            Add {
                lhs: SSAValue,
                rhs: SSAValue,
                #[kirin(type = SimpleType::Int)]
                res: ResultValue,
            },
            #[chumsky(format = "return {arg}")]
            #[kirin(terminator)]
            Return {
                arg: SSAValue,
            },
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("enum_simple_ast", ast);
    insta::assert_snapshot!("enum_simple_emit", emit);
    insta::assert_snapshot!("enum_simple_parser", parser);
    insta::assert_snapshot!("enum_simple_pretty", pretty);
}

#[test]
fn test_enum_tuple_variants() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        enum TupleLang {
            #[chumsky(format = "return {0}")]
            #[kirin(terminator)]
            Return(SSAValue),
            #[chumsky(format = "{1:name} = neg {0} -> {1:type}")]
            Neg(SSAValue, #[kirin(type = SimpleType::Int)] ResultValue),
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("enum_tuple_variants_ast", ast);
    insta::assert_snapshot!("enum_tuple_variants_emit", emit);
    insta::assert_snapshot!("enum_tuple_variants_parser", parser);
    insta::assert_snapshot!("enum_tuple_variants_pretty", pretty);
}

// =============================================================================
// Wrapper Tests
// =============================================================================

/// Test struct wrapper - currently ignored due to a bug in AST generation
/// where wrapper structs generate empty tuple fields resulting in invalid syntax.
/// TODO: Fix the AST generator to properly handle wrapper structs.
#[test]
#[ignore = "Bug: wrapper structs generate empty tuple fields, see ast.rs"]
fn test_struct_wrapper() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[wraps]
        struct MyWrapper(OtherDialect);
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("struct_wrapper_ast", ast);
    insta::assert_snapshot!("struct_wrapper_emit", emit);
    insta::assert_snapshot!("struct_wrapper_parser", parser);
    insta::assert_snapshot!("struct_wrapper_pretty", pretty);
}

#[test]
fn test_enum_all_wrappers() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[wraps]
        enum ComposedLang {
            Arith(ArithDialect),
            Control(ControlDialect),
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("enum_all_wrappers_ast", ast);
    insta::assert_snapshot!("enum_all_wrappers_emit", emit);
    insta::assert_snapshot!("enum_all_wrappers_parser", parser);
    insta::assert_snapshot!("enum_all_wrappers_pretty", pretty);
}

#[test]
fn test_enum_mixed_wrappers() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        enum MixedLang {
            #[wraps]
            Arith(ArithDialect),
            #[chumsky(format = "{res:name} = custom {arg} -> {res:type}")]
            Custom {
                arg: SSAValue,
                #[kirin(type = SimpleType::Int)]
                res: ResultValue,
            },
            #[wraps]
            Control(ControlDialect),
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("enum_mixed_wrappers_ast", ast);
    insta::assert_snapshot!("enum_mixed_wrappers_emit", emit);
    insta::assert_snapshot!("enum_mixed_wrappers_parser", parser);
    insta::assert_snapshot!("enum_mixed_wrappers_pretty", pretty);
}

// =============================================================================
// Complex Field Types Tests
// =============================================================================

#[test]
fn test_struct_with_block() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "br {target}")]
        #[kirin(terminator)]
        struct Branch {
            target: Successor,
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("struct_with_block_ast", ast);
    insta::assert_snapshot!("struct_with_block_emit", emit);
    insta::assert_snapshot!("struct_with_block_parser", parser);
    insta::assert_snapshot!("struct_with_block_pretty", pretty);
}

#[test]
fn test_struct_conditional_branch() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "br {cond}, {then_target}, {else_target}")]
        #[kirin(terminator)]
        struct CondBranch {
            cond: SSAValue,
            then_target: Successor,
            else_target: Successor,
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("struct_cond_branch_ast", ast);
    insta::assert_snapshot!("struct_cond_branch_emit", emit);
    insta::assert_snapshot!("struct_cond_branch_parser", parser);
    insta::assert_snapshot!("struct_cond_branch_pretty", pretty);
}

// =============================================================================
// Generic Type Tests
// =============================================================================

#[test]
fn test_struct_with_generics() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = T)]
        #[chumsky(format = "return {0}")]
        struct GenericReturn<T: TypeLattice>(SSAValue);
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("struct_with_generics_ast", ast);
    insta::assert_snapshot!("struct_with_generics_emit", emit);
    insta::assert_snapshot!("struct_with_generics_parser", parser);
    insta::assert_snapshot!("struct_with_generics_pretty", pretty);
}

#[test]
fn test_enum_wrapper_with_generics() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = T)]
        #[wraps]
        enum WrapperWithGenerics<T: TypeLattice> {
            Inner(GenericInner<T>),
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("enum_wrapper_with_generics_ast", ast);
    insta::assert_snapshot!("enum_wrapper_with_generics_emit", emit);
    insta::assert_snapshot!("enum_wrapper_with_generics_parser", parser);
    insta::assert_snapshot!("enum_wrapper_with_generics_pretty", pretty);
}

// =============================================================================
// Custom Crate Path Tests
// =============================================================================

#[test]
fn test_custom_crate_path() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, crate = "my_kirin")]
        #[chumsky(crate = "my_chumsky", format = "return {0}")]
        struct CustomReturn(SSAValue);
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("custom_crate_path_ast", ast);
    insta::assert_snapshot!("custom_crate_path_emit", emit);
    insta::assert_snapshot!("custom_crate_path_parser", parser);
    insta::assert_snapshot!("custom_crate_path_pretty", pretty);
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_empty_enum() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        enum EmptyLang {}
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("empty_enum_ast", ast);
    insta::assert_snapshot!("empty_enum_emit", emit);
    insta::assert_snapshot!("empty_enum_parser", parser);
    insta::assert_snapshot!("empty_enum_pretty", pretty);
}

#[test]
fn test_format_with_special_tokens() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "{res:name} = load [{addr}] -> {res:type}")]
        struct Load {
            addr: SSAValue,
            #[kirin(type = SimpleType::Int)]
            res: ResultValue,
        }
    };
    let (ast, emit, parser, pretty) = generate_all(&input);
    insta::assert_snapshot!("format_special_tokens_ast", ast);
    insta::assert_snapshot!("format_special_tokens_emit", emit);
    insta::assert_snapshot!("format_special_tokens_parser", parser);
    insta::assert_snapshot!("format_special_tokens_pretty", pretty);
}
