use kirin_derive_toolkit::context::{DeriveContext, StatementContext};
use kirin_derive_toolkit::ir::Input;
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{AssocTypeSpec, Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::EvalCallLayout;

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";
const DEFAULT_IR_CRATE: &str = "::kirin::ir";

pub fn do_derive_eval_call(input: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<EvalCallLayout>::from_derive_input(input)?;

    // Validate that at least one variant has #[callable] (or #[callable] is on the enum itself).
    let callable_all = ir.extra_attrs.callable;
    let any_variant_callable = match &ir.data {
        kirin_derive_toolkit::ir::Data::Enum(data) => {
            data.variants.iter().any(|s| s.extra_attrs.callable)
        }
        kirin_derive_toolkit::ir::Data::Struct(data) => data.0.extra_attrs.callable,
    };
    if !callable_all && !any_variant_callable {
        return Err(darling::Error::custom(
            "derive(CallSemantics) requires at least one #[callable] variant",
        ));
    }

    let ir_crate: syn::Path = ir
        .attrs
        .crate_path
        .clone()
        .unwrap_or_else(|| from_str(DEFAULT_IR_CRATE));
    let template = TraitImplTemplate::new(
        syn::parse_quote!(::kirin_interpreter::CallSemantics),
        from_str(DEFAULT_INTERP_CRATE),
    )
    .generics_modifier(|base| {
        let mut generics = base.clone();
        generics
            .params
            .insert(0, syn::GenericParam::Lifetime(syn::parse_quote!('__ir)));
        generics
            .params
            .push(syn::GenericParam::Type(syn::parse_quote!(__CallSemI)));
        generics
    })
    .trait_generics(|_ctx| {
        quote! { <'__ir, __CallSemI> }
    })
    .where_clause(|ctx| {
        let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);

        let mut predicates: Vec<syn::WherePredicate> = vec![
            syn::parse_quote! { __CallSemI: #interp_crate::Interpreter<'__ir> },
            syn::parse_quote! { __CallSemI::Error: From<#interp_crate::InterpreterError> },
        ];

        let callable_wrappers = collect_callable_wrappers(ctx);
        let result_type = if let Some(first_ty) = callable_wrappers.first() {
            quote! { <#first_ty as #interp_crate::CallSemantics<'__ir, __CallSemI>>::Result }
        } else {
            quote! { __CallSemI::Value }
        };

        for (i, wrapper_ty) in callable_wrappers.iter().enumerate() {
            if i == 0 {
                predicates.push(syn::parse_quote! {
                    #wrapper_ty: #interp_crate::CallSemantics<'__ir, __CallSemI>
                });
            } else {
                predicates.push(syn::parse_quote! {
                    #wrapper_ty: #interp_crate::CallSemantics<'__ir, __CallSemI, Result = #result_type>
                });
            }
        }

        Some(syn::parse_quote! { where #(#predicates),* })
    })
    .assoc_type(AssocTypeSpec::PerStatement {
        name: format_ident!("Result"),
        compute: Box::new(|ctx, _first_stmt| {
            let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);

            let callable_wrappers = collect_callable_wrappers(ctx);
            if let Some(first_ty) = callable_wrappers.first() {
                quote! { <#first_ty as #interp_crate::CallSemantics<'__ir, __CallSemI>>::Result }
            } else {
                quote! { __CallSemI::Value }
            }
        }),
    })
    .method({
        let ir_crate_m = ir_crate.clone();
        let interp_crate_m: syn::Path = from_str(DEFAULT_INTERP_CRATE);
        MethodSpec {
            name: format_ident!("eval_call"),
            self_arg: quote! { &self },
            params: vec![
                quote! { interpreter: &mut __CallSemI },
                quote! { stage: &'__ir #ir_crate::StageInfo<__CallSemL> },
                quote! { callee: #ir_crate::SpecializedFunction },
                quote! { args: &[__CallSemI::Value] },
            ],
            return_type: Some(quote! { Result<Self::Result, __CallSemI::Error> }),
            pattern: Box::new(Custom::separate(
                // for_struct
                |_ctx, stmt_ctx| {
                    let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
                    if stmt_ctx.is_wrapper {
                        let pattern = &stmt_ctx.pattern;
                        let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();
                        Ok(quote! {
                            let Self #pattern = self;
                            #binding.eval_call::<__CallSemL>(interpreter, stage, callee, args)
                        })
                    } else {
                        Ok(quote! {
                            Err(#interp_crate::InterpreterError::missing_function_entry().into())
                        })
                    }
                },
                // for_variant
                |ctx, stmt_ctx| {
                    let interp_crate: syn::Path = from_str(DEFAULT_INTERP_CRATE);
                    if is_call_forwarding(ctx, stmt_ctx) {
                        let binding = stmt_ctx.wrapper_binding.as_ref().unwrap();
                        Ok(quote! { #binding.eval_call::<__CallSemL>(interpreter, stage, callee, args) })
                    } else {
                        Ok(quote! { Err(#interp_crate::InterpreterError::missing_function_entry().into()) })
                    }
                },
            )),
            generics: Some(quote! { <__CallSemL: #ir_crate_m::Dialect> }),
            method_where_clause: Some(quote! {
                where
                    __CallSemI::StageInfo: #ir_crate_m::HasStageInfo<__CallSemL>,
                    __CallSemI::Error: From<#interp_crate_m::InterpreterError>,
                    __CallSemL: #interp_crate_m::Interpretable<'__ir, __CallSemI>
                        + #interp_crate_m::CallSemantics<'__ir, __CallSemI, Result = Self::Result>
                        + '__ir
            }),
        }
    });

    ir.compose().add(template).build()
}

/// Determine if a variant should forward eval_call.
/// A variant forwards only if it has `#[wraps]` AND (`#[callable]` on itself or on the enum).
fn is_call_forwarding(
    ctx: &DeriveContext<'_, EvalCallLayout>,
    stmt_ctx: &StatementContext<'_, EvalCallLayout>,
) -> bool {
    let callable_all = ctx.input.extra_attrs.callable;
    let is_callable = callable_all || stmt_ctx.stmt.extra_attrs.callable;
    stmt_ctx.is_wrapper && is_callable
}

/// Collect wrapper types that should forward eval_call.
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

    fn generate_call_semantics_code(input: syn::DeriveInput) -> String {
        let tokens = do_derive_eval_call(&input).expect("Failed to generate CallSemantics");
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_call_semantics_with_callable() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum FuncOps {
                #[wraps]
                #[callable]
                Call(CallOp),
                #[wraps]
                Return(ReturnOp),
            }
        };
        insta::assert_snapshot!(generate_call_semantics_code(input));
    }

    #[test]
    fn test_call_semantics_without_callable() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum FuncOps {
                #[wraps]
                Call(CallOp),
                #[wraps]
                Return(ReturnOp),
            }
        };
        let err = do_derive_eval_call(&input).unwrap_err();
        assert!(
            err.to_string()
                .contains("derive(CallSemantics) requires at least one #[callable] variant"),
            "unexpected error: {err}",
        );
    }

    #[test]
    fn test_call_semantics_all_callable() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[callable]
            enum FuncOps {
                #[wraps]
                Call(CallOp),
                #[wraps]
                Invoke(InvokeOp),
            }
        };
        insta::assert_snapshot!(generate_call_semantics_code(input));
    }

    #[test]
    fn test_call_semantics_mixed_callable_non_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum FuncOps {
                #[wraps]
                #[callable]
                Call(CallOp),
                #[wraps]
                Return(ReturnOp),
                #[wraps]
                #[callable]
                Invoke(InvokeOp),
            }
        };
        insta::assert_snapshot!(generate_call_semantics_code(input));
    }

    #[test]
    fn test_call_semantics_struct_callable_wraps() {
        // Struct with both #[callable] and #[wraps]
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[callable]
            #[wraps]
            struct CallOp(InnerCall);
        };
        insta::assert_snapshot!(generate_call_semantics_code(input));
    }

    #[test]
    fn test_call_semantics_struct_callable_without_wraps() {
        // Struct with #[callable] but without #[wraps] — produces error fallback
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[callable]
            struct DirectCall {
                target: Symbol,
            }
        };
        insta::assert_snapshot!(generate_call_semantics_code(input));
    }

    #[test]
    fn test_call_semantics_enum_all_callable_with_non_wraps_variant() {
        // Enum-level #[callable] but some variants are not #[wraps]
        // Should generate error fallback for non-wraps variants
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[callable]
            enum FuncOps {
                #[wraps]
                Call(CallOp),
                Plain { value: i64 },
            }
        };
        insta::assert_snapshot!(generate_call_semantics_code(input));
    }

    #[test]
    fn test_call_semantics_single_callable_variant() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum FuncOps {
                #[wraps]
                #[callable]
                Call(CallOp),
            }
        };
        insta::assert_snapshot!(generate_call_semantics_code(input));
    }

    #[test]
    fn test_call_semantics_struct_not_callable_error() {
        // Struct without #[callable] should error
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[wraps]
            struct WrapperOp(InnerOp);
        };
        let result = do_derive_eval_call(&input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("callable"),
            "Error should mention #[callable] requirement: {err}"
        );
    }
}
