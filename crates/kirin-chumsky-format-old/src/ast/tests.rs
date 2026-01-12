use super::DeriveChumskyAst;
use crate::ChumskyLayout;

fn derive_ast(input: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let ir_input =
        kirin_derive_core_2::ir::Input::<ChumskyLayout>::from_derive_input(input).unwrap();
    DeriveChumskyAst::new(&ir_input).generate(&ir_input)
}

#[test]
fn generates_ast_enum_using_parser_nodes() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        enum Simple {
            #[chumsky(format = "add {0} {1}")]
            Add(kirin_ir::SSAValue, kirin_ir::SSAValue, kirin_ir::ResultValue),
            #[chumsky(format = "return {0}")]
            Return(kirin_ir::SSAValue),
        }
    };

    let tokens = derive_ast(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn generates_ast_for_named_struct() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        #[chumsky(format = "complex {first} {results} {maybe_block}")]
        struct Complex {
            first: kirin_ir::SSAValue,
            results: Vec<kirin_ir::ResultValue>,
            maybe_block: Option<kirin_ir::Block>,
            #[kirin(default = 0)]
            ignored_default: u32,
        }
    };

    let tokens = derive_ast(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn generates_ast_for_tuple_variants() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        enum Flow {
            #[chumsky(format = "wrap {0}")]
            Wrap(kirin_ir::Region),
            #[chumsky(format = "branch {0} {1}")]
            Branch(kirin_ir::SSAValue, kirin_ir::Successor),
            #[chumsky(format = "done")]
            Done,
        }
    };

    let tokens = derive_ast(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn generates_ast_with_named_and_typed_value() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        #[chumsky(format = "sin {0:name} -> {0:type}")]
        struct Sin(kirin_ir::SSAValue, kirin_ir::ResultValue);
    };

    let tokens = derive_ast(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn generates_ast_with_result_type_and_ssa_defaults() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        #[chumsky(format = "add {lhs} {rhs} -> {result:type}")]
        struct Add {
            lhs: kirin_ir::SSAValue,
            rhs: kirin_ir::SSAValue,
            result: kirin_ir::ResultValue,
        }
    };

    let tokens = derive_ast(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn errors_when_format_missing() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        struct Missing(kirin_ir::SSAValue);
    };

    let tokens = derive_ast(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    assert!(
        formatted.contains("chumsky format specification is required"),
        "expected compile_error for missing format, got {formatted}"
    );
}

#[test]
fn generates_ast_for_custom_parsed_types() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        #[chumsky(format = "const {value}")]
        struct Const {
            value: u64,
        }
    };

    let tokens = derive_ast(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}
