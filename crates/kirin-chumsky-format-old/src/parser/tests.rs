use crate::{parser::DeriveChumskyParser, ChumskyLayout};

fn derive_parser(input: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let ir_input =
        kirin_derive_core_2::ir::Input::<ChumskyLayout>::from_derive_input(input).unwrap();
    DeriveChumskyParser::new(&ir_input).generate(&ir_input)
}

#[test]
fn generates_parser_with_custom_crate_path() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        #[chumsky(crate = custom_chumsky::parser, format = "id {0}")]
        struct Id(kirin_ir::SSAValue);
    };

    let tokens = derive_parser(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn generates_parser_for_result_typed_add() {
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

    let tokens = derive_parser(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn generates_parser_for_enum_variants() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        enum Math {
            #[chumsky(format = "add {lhs} {rhs} -> {result:type}")]
            Add {
                lhs: kirin_ir::SSAValue,
                rhs: kirin_ir::SSAValue,
                result: kirin_ir::ResultValue,
            },
            #[chumsky(format = "sub {lhs} {rhs} -> {result:type}")]
            Sub {
                lhs: kirin_ir::SSAValue,
                rhs: kirin_ir::SSAValue,
                result: kirin_ir::ResultValue,
            },
        }
    };

    let tokens = derive_parser(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn generates_parser_for_cf_dialect() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = Type)]
        enum Cf {
            #[chumsky(format = "br {target}")]
            Br {
                target: kirin_ir::Successor,
            },
            #[chumsky(format = "cond_br {cond:name} then={then_target} else={else_target}")]
            CondBr {
                cond: kirin_ir::SSAValue,
                then_target: kirin_ir::Successor,
                else_target: kirin_ir::Successor,
            },
        }
    };

    let tokens = derive_parser(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}

#[test]
fn generates_parser_for_function_like_stmt() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone)]
        #[kirin(type_lattice = L)]
        #[chumsky(format = "fn {name}({input_signature}) -> {ret_type} {body}")]
        struct Func<L: kirin_ir::TypeLattice> {
            name: String,
            input_signature: InputSignature,
            ret_type: L,
            body: kirin_ir::Region,
        }
    };

    let tokens = derive_parser(&input);
    let formatted = kirin_derive_core_2::test_util::rustfmt_tokens(&tokens);
    insta::assert_snapshot!(formatted);
}
