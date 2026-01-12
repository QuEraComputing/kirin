use kirin_chumsky_format::parser::DeriveChumskyParser;

fn fmt(tokens: proc_macro2::TokenStream) -> String {
    kirin_derive_core_2::test_util::rustfmt_tokens(&tokens)
}

fn generate(input: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let ir_input =
        kirin_derive_core_2::ir::Input::<kirin_chumsky_format::ChumskyLayout>::from_derive_input(input).unwrap();

    DeriveChumskyParser::new(&ir_input).generate(&ir_input)
}

#[test]
fn derives_parser_for_struct() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone, WithRecursiveChumskyParser)]
        #[kirin(type_lattice = Type)]
        #[chumsky(format = "add {lhs} {rhs} -> {result:type}")]
        struct Add {
            lhs: kirin_ir::SSAValue,
            rhs: kirin_ir::SSAValue,
            result: kirin_ir::ResultValue,
        }
    };

    let output = generate(&input);
    let formatted = fmt(output);
    insta::assert_snapshot!(formatted);
}

#[test]
fn derives_parser_for_enum_variants() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[derive(Clone, WithRecursiveChumskyParser)]
        #[kirin(type_lattice = Type)]
        enum Cf {
            #[chumsky(format = "br {target}")]
            Br { target: kirin_ir::Successor },
            #[chumsky(format = "cond_br {cond:name} then={then_target} else={else_target}")]
            CondBr {
                cond: kirin_ir::SSAValue,
                then_target: kirin_ir::Successor,
                else_target: kirin_ir::Successor,
            },
        }
    };

    let output = generate(&input);
    let formatted = fmt(output);
    insta::assert_snapshot!(formatted);
}
