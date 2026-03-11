use kirin_derive_toolkit::context::{DeriveContext, StatementContext};
use kirin_derive_toolkit::ir::Input;
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::quote;

use crate::eval_call::EvalCallLayout;

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";
const DEFAULT_IR_CRATE: &str = "::kirin::ir";

pub fn do_derive_ssa_cfg_region(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<EvalCallLayout>::from_derive_input(input)?;

    // Validate that at least one variant has #[callable].
    let callable_all = ir.extra_attrs.callable;
    let any_variant_callable = match &ir.data {
        kirin_derive_toolkit::ir::Data::Enum(data) => {
            data.variants.iter().any(|s| s.extra_attrs.callable)
        }
        kirin_derive_toolkit::ir::Data::Struct(data) => data.0.extra_attrs.callable,
    };
    if !callable_all && !any_variant_callable {
        return Err(darling::Error::custom(
            "derive(SSACFGRegion) requires at least one #[callable] variant",
        ));
    }

    let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
    let ir_crate: syn::Path = ir
        .attrs
        .crate_path
        .clone()
        .unwrap_or_else(|| from_str(DEFAULT_IR_CRATE));

    let template = TraitImplTemplate::new(
        syn::parse_quote!(::kirin_interpreter::SSACFGRegion),
        interp_crate.clone(),
    )
    .where_clause({
        let interp_crate = interp_crate.clone();
        move |ctx| {
            let callable_wrappers = collect_callable_wrappers(ctx);
            if callable_wrappers.is_empty() {
                return None;
            }
            let predicates: Vec<syn::WherePredicate> = callable_wrappers
                .iter()
                .map(|ty| -> syn::WherePredicate {
                    syn::parse_quote! { #ty: #interp_crate::SSACFGRegion }
                })
                .collect();
            Some(syn::parse_quote! { where #(#predicates),* })
        }
    })
    .method({
        let ir_crate_m = ir_crate.clone();
        MethodSpec {
            name: syn::parse_quote!(entry_block),
            self_arg: quote! { &self },
            params: vec![quote! { stage: &#ir_crate_m::StageInfo<__L> }],
            return_type: Some(
                quote! { Result<#ir_crate_m::Block, #interp_crate::InterpreterError> },
            ),
            pattern: Box::new(Custom::separate(
                // for_struct
                |_ctx, stmt_ctx| {
                    let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
                    if stmt_ctx.is_wrapper {
                        let pattern = &stmt_ctx.pattern;
                        let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();
                        Ok(quote! {
                            let Self #pattern = self;
                            #binding.entry_block(stage)
                        })
                    } else {
                        Ok(quote! {
                            Err(#interp_crate::InterpreterError::missing_entry_block())
                        })
                    }
                },
                // for_variant
                |ctx, stmt_ctx| {
                    let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
                    if is_call_forwarding(ctx, stmt_ctx) {
                        let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();
                        Ok(quote! { #binding.entry_block(stage) })
                    } else {
                        Ok(quote! { Err(#interp_crate::InterpreterError::missing_entry_block()) })
                    }
                },
            )),
            generics: Some(quote! { <__L: #ir_crate_m::Dialect> }),
            method_where_clause: None,
        }
    });

    ir.compose().add(template).build()
}

/// Determine if a variant should forward entry_block.
/// A variant forwards only if it has `#[wraps]` AND (`#[callable]` on itself or on the enum).
fn is_call_forwarding(
    ctx: &DeriveContext<'_, EvalCallLayout>,
    stmt_ctx: &StatementContext<'_, EvalCallLayout>,
) -> bool {
    let callable_all = ctx.input.extra_attrs.callable;
    let is_callable = callable_all || stmt_ctx.stmt.extra_attrs.callable;
    stmt_ctx.is_wrapper && is_callable
}

/// Collect wrapper types that should forward entry_block.
fn collect_callable_wrappers<'a>(ctx: &'a DeriveContext<'_, EvalCallLayout>) -> Vec<&'a syn::Type> {
    ctx.statements
        .values()
        .filter(|stmt_ctx| is_call_forwarding(ctx, stmt_ctx))
        .filter_map(|stmt_ctx| stmt_ctx.wrapper_type)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    fn generate_ssa_cfg_region_code(input: syn::DeriveInput) -> String {
        let tokens = do_derive_ssa_cfg_region(&input).expect("Failed to generate SSACFGRegion");
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_ssa_cfg_region_with_callable() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum MyLang {
                #[wraps]
                #[callable]
                Lexical(LexicalOp),
                #[wraps]
                Arith(ArithOp),
            }
        };
        insta::assert_snapshot!(generate_ssa_cfg_region_code(input));
    }

    #[test]
    fn test_ssa_cfg_region_all_callable() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[callable]
            enum MyLang {
                #[wraps]
                Lexical(LexicalOp),
                #[wraps]
                Lifted(LiftedOp),
            }
        };
        insta::assert_snapshot!(generate_ssa_cfg_region_code(input));
    }

    #[test]
    fn test_ssa_cfg_region_without_callable_error() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum MyLang {
                #[wraps]
                Arith(ArithOp),
            }
        };
        let err = do_derive_ssa_cfg_region(&input).unwrap_err();
        assert!(
            err.to_string()
                .contains("derive(SSACFGRegion) requires at least one #[callable] variant"),
            "unexpected error: {err}",
        );
    }

    #[test]
    fn test_ssa_cfg_region_struct_callable_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[callable]
            #[wraps]
            struct FuncBody(InnerBody);
        };
        insta::assert_snapshot!(generate_ssa_cfg_region_code(input));
    }
}
