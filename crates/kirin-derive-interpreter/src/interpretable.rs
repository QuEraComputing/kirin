use kirin_derive_toolkit::ir::{Input, StandardLayout};
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::quote;

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";
const DEFAULT_IR_CRATE: &str = "::kirin::ir";

pub fn do_derive_interpretable(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(input)?;
    let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
    let ir_crate: syn::Path = ir
        .attrs
        .crate_path
        .clone()
        .unwrap_or_else(|| from_str(DEFAULT_IR_CRATE));

    let template = TraitImplTemplate::new(
        syn::parse_quote!(::kirin_interpreter::Interpretable),
        interp_crate.clone(),
    )
    .generics_modifier(|base| {
        let mut generics = base.clone();
        generics
            .params
            .insert(0, syn::GenericParam::Lifetime(syn::parse_quote!('__ir)));
        generics
            .params
            .push(syn::GenericParam::Type(syn::parse_quote!(__InterpI)));
        generics
    })
    .trait_generics(|_ctx| quote! { <'__ir, __InterpI> })
    .where_clause({
        let interp_crate = interp_crate.clone();
        move |ctx| {
            let mut predicates: Vec<syn::WherePredicate> = vec![
                syn::parse_quote! { __InterpI: #interp_crate::Interpreter<'__ir> },
            ];
            for stmt_ctx in ctx.statements.values() {
                if let Some(wrapper_ty) = stmt_ctx.wrapper_type {
                    predicates.push(syn::parse_quote! {
                        #wrapper_ty: #interp_crate::Interpretable<'__ir, __InterpI>
                    });
                }
            }
            Some(syn::parse_quote! { where #(#predicates),* })
        }
    })
    .validate(|ctx| {
        let non_wrappers: Vec<_> = ctx
            .statements
            .values()
            .filter(|s| !s.is_wrapper)
            .map(|s| s.stmt.name.to_string())
            .collect();
        if !non_wrappers.is_empty() {
            return Err(darling::Error::custom(format!(
                "Cannot derive `Interpretable`: variant(s) {} are not `#[wraps]`. \
                 Either implement `Interpretable` manually, or wrap each variant with `#[wraps]`.",
                non_wrappers.join(", "),
            )));
        }
        Ok(())
    })
    .method({
        let interp_crate_m = interp_crate.clone();
        let ir_crate_m = ir_crate.clone();
        MethodSpec {
            name: syn::parse_quote!(interpret),
            self_arg: quote! { &self },
            params: vec![quote! { interpreter: &mut __InterpI }],
            return_type: Some({
                let interp_crate = interp_crate.clone();
                quote! { Result<#interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error> }
            }),
            pattern: Box::new(Custom::new(|_ctx, stmt_ctx| {
                let binding = stmt_ctx
                    .wrapper_binding
                    .as_ref()
                    .ok_or_else(|| darling::Error::custom("expected wrapper binding"))?;
                Ok(quote! { #binding.interpret::<__InterpL>(interpreter) })
            })),
            generics: Some(quote! { <__InterpL> }),
            method_where_clause: Some(quote! {
                where
                    __InterpI::StageInfo: #ir_crate_m::HasStageInfo<__InterpL>,
                    __InterpI::Error: From<#interp_crate_m::InterpreterError>,
                    __InterpL: #interp_crate_m::Interpretable<'__ir, __InterpI> + '__ir
            }),
        }
    });

    ir.compose().add(template).build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    fn generate_interpretable_code(input: syn::DeriveInput) -> String {
        let tokens = do_derive_interpretable(&input).expect("Failed to generate Interpretable");
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_interpretable_enum_all_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum ArithOps {
                #[wraps]
                Add(AddOp),
                #[wraps]
                Sub(SubOp),
                #[wraps]
                Mul(MulOp),
            }
        };
        insta::assert_snapshot!(generate_interpretable_code(input));
    }

    #[test]
    fn test_interpretable_validation_error_non_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum MixedOps {
                #[wraps]
                Add(AddOp),
                Literal { value: i64 },
            }
        };
        let result = do_derive_interpretable(&input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Literal"),
            "Error should mention the non-wraps variant: {err}"
        );
    }

    #[test]
    fn test_interpretable_single_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum SingleOp {
                #[wraps]
                Only(OnlyOp),
            }
        };
        insta::assert_snapshot!(generate_interpretable_code(input));
    }

    #[test]
    fn test_interpretable_all_non_wraps_error() {
        // No variants have #[wraps]
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum PlainOps {
                Lit { value: i64 },
                Nop {},
            }
        };
        let result = do_derive_interpretable(&input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Lit"),
            "Error should mention the non-wraps variant Lit: {err}"
        );
        assert!(
            err.contains("Nop"),
            "Error should mention the non-wraps variant Nop: {err}"
        );
    }

    #[test]
    fn test_interpretable_struct_wraps() {
        // Struct with #[wraps] — should succeed as it's a wrapper
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[wraps]
            struct WrappedOp(InnerOp);
        };
        insta::assert_snapshot!(generate_interpretable_code(input));
    }

    #[test]
    fn test_interpretable_struct_without_wraps_error() {
        // Struct without #[wraps] — should error
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct PlainStruct {
                value: i64,
            }
        };
        let result = do_derive_interpretable(&input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("PlainStruct"),
            "Error should mention the struct name: {err}"
        );
    }

    #[test]
    fn test_interpretable_many_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum BigOps {
                #[wraps]
                A(AOp),
                #[wraps]
                B(BOp),
                #[wraps]
                C(COp),
                #[wraps]
                D(DOp),
                #[wraps]
                E(EOp),
            }
        };
        insta::assert_snapshot!(generate_interpretable_code(input));
    }

    #[test]
    fn test_interpretable_enum_level_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[wraps]
            enum Composed {
                A(AOp),
                B(BOp),
            }
        };
        insta::assert_snapshot!(generate_interpretable_code(input));
    }

    #[test]
    fn test_interpretable_enum_level_wraps_with_generics() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = T)]
            #[wraps]
            enum Lexical<T: CompileTimeValue> {
                FunctionBody(FunctionBody<T>),
                Lambda(Lambda<T>),
                Call(Call<T>),
                Return(Return<T>),
            }
        };
        insta::assert_snapshot!(generate_interpretable_code(input));
    }
}
