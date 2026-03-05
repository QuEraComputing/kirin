use super::{DeriveEvalCall, EvalCallLayout};
use kirin_derive_toolkit::codegen::combine_where_clauses;
use kirin_derive_toolkit::emit::Emit;
use kirin_derive_toolkit::ir;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::tokens::{MatchArm, MatchExpr, Method, TraitImpl};
use quote::{format_ident, quote};

impl<'ir> Emit<'ir, EvalCallLayout> for DeriveEvalCall {
    fn emit_struct(
        &mut self,
        data: &'ir ir::DataStruct<EvalCallLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let info = self.statement_info(&data.0)?;
        let interp_crate = self.interpreter_crate_path();
        let ir_crate = self.ir_crate_path(input);
        let type_name = &input.core.name;
        let (_, ty_generics, orig_where) = input.core.generics.split_for_impl();

        let generics = add_interpreter_param(&input.core.generics);

        let trait_path = quote! { #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics> };

        let eval_call_params = vec![
            quote! { interpreter: &mut __CallSemI },
            quote! { stage: &'__ir #ir_crate::StageInfo<#type_name #ty_generics> },
            quote! { callee: #ir_crate::SpecializedFunction },
            quote! { args: &[__CallSemI::Value] },
        ];

        if info.is_wrapper {
            let wrapper_ty = info.wrapper_ty.as_ref().unwrap();
            let pattern = &info.pattern;
            let binding = info.wrapper_binding.as_ref().unwrap();

            let extra_where: syn::WhereClause = syn::parse_quote! {
                where
                    __CallSemI: #interp_crate::Interpreter<'__ir>,
                    __CallSemI::Error: From<#interp_crate::InterpreterError>,
                    #wrapper_ty: #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>
            };

            let result_type = quote! {
                <#wrapper_ty as #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>>::Result
            };

            let trait_impl = TraitImpl::new(generics, &trait_path, type_name)
                .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
                .assoc_type(format_ident!("Result"), &result_type)
                .method(Method {
                    name: format_ident!("eval_call"),
                    self_arg: quote! { &self },
                    params: eval_call_params,
                    return_type: Some(quote! { Result<Self::Result, __CallSemI::Error> }),
                    body: quote! {
                        let Self #pattern = self;
                        #binding.eval_call(interpreter, stage, callee, args)
                    },
                });

            Ok(quote! { #trait_impl })
        } else {
            let extra_where: syn::WhereClause = syn::parse_quote! {
                where
                    __CallSemI: #interp_crate::Interpreter<'__ir>,
                    __CallSemI::Error: From<#interp_crate::InterpreterError>
            };

            let trait_impl = TraitImpl::new(generics, &trait_path, type_name)
                .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
                .assoc_type(format_ident!("Result"), quote! { __CallSemI::Value })
                .method(Method {
                    name: format_ident!("eval_call"),
                    self_arg: quote! { &self },
                    params: vec![
                        quote! { _interpreter: &mut __CallSemI },
                        quote! { _stage: &'__ir #ir_crate::StageInfo<#type_name #ty_generics> },
                        quote! { _callee: #ir_crate::SpecializedFunction },
                        quote! { _args: &[__CallSemI::Value] },
                    ],
                    return_type: Some(quote! { Result<Self::Result, __CallSemI::Error> }),
                    body: quote! {
                        Err(#interp_crate::InterpreterError::MissingEntry.into())
                    },
                });

            Ok(quote! { #trait_impl })
        }
    }

    fn emit_enum(
        &mut self,
        data: &'ir ir::DataEnum<EvalCallLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let interp_crate = self.interpreter_crate_path();
        let ir_crate = self.ir_crate_path(input);
        let type_name = &input.core.name;
        let (_, ty_generics, orig_where) = input.core.generics.split_for_impl();

        let generics = add_interpreter_param(&input.core.generics);

        // Determine if #[callable] is used anywhere (enum-level or any variant).
        let any_callable =
            input.callable_all || self.statements.values().any(|info| info.is_callable);

        let mut wrapper_types: Vec<&syn::Type> = Vec::new();
        let mut match_arms = Vec::new();

        for variant in &data.variants {
            let info = self.statement_info(variant)?;
            let variant_name = &info.name;
            let pattern = &info.pattern;

            // A variant forwards eval_call if:
            // - No #[callable] used anywhere: fall back to #[wraps] (backward compat)
            // - #[callable] used: only callable wrappers forward
            let is_call_wrapper = if any_callable {
                info.is_wrapper && info.is_callable
            } else {
                info.is_wrapper
            };

            if is_call_wrapper {
                let wrapper_ty = info.wrapper_ty.as_ref().unwrap();
                wrapper_types.push(wrapper_ty);
                let binding = info.wrapper_binding.as_ref().unwrap();

                match_arms.push(MatchArm {
                    pattern: quote! { Self::#variant_name #pattern },
                    guard: None,
                    body: quote! { #binding.eval_call(interpreter, stage, callee, args) },
                });
            } else if info.pattern.is_empty() {
                match_arms.push(MatchArm {
                    pattern: quote! { Self::#variant_name },
                    guard: None,
                    body: quote! { Err(#interp_crate::InterpreterError::MissingEntry.into()) },
                });
            } else {
                match_arms.push(MatchArm {
                    pattern: quote! { Self::#variant_name #pattern },
                    guard: None,
                    body: quote! { Err(#interp_crate::InterpreterError::MissingEntry.into()) },
                });
            }
        }

        let result_type = if let Some(first_wrapper) = wrapper_types.first() {
            quote! { <#first_wrapper as #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics>>::Result }
        } else {
            quote! { __CallSemI::Value }
        };

        let where_bounds: Vec<proc_macro2::TokenStream> = wrapper_types
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                if i == 0 {
                    quote! { #ty: #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics> }
                } else {
                    quote! { #ty: #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics, Result = #result_type> }
                }
            })
            .collect();

        let extra_where: syn::WhereClause = syn::parse_quote! {
            where
                __CallSemI: #interp_crate::Interpreter<'__ir>,
                __CallSemI::Error: From<#interp_crate::InterpreterError>,
                #(#where_bounds),*
        };

        let trait_path = quote! { #interp_crate::CallSemantics<'__ir, __CallSemI, #type_name #ty_generics> };

        let match_expr = MatchExpr {
            subject: quote! { self },
            arms: match_arms,
        };

        let trait_impl = TraitImpl::new(generics, &trait_path, type_name)
            .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
            .assoc_type(format_ident!("Result"), &result_type)
            .method(Method {
                name: format_ident!("eval_call"),
                self_arg: quote! { &self },
                params: vec![
                    quote! { interpreter: &mut __CallSemI },
                    quote! { stage: &'__ir #ir_crate::StageInfo<#type_name #ty_generics> },
                    quote! { callee: #ir_crate::SpecializedFunction },
                    quote! { args: &[__CallSemI::Value] },
                ],
                return_type: Some(quote! { Result<Self::Result, __CallSemI::Error> }),
                body: quote! { #match_expr },
            });

        Ok(quote! { #trait_impl })
    }
}

fn add_interpreter_param(base: &syn::Generics) -> syn::Generics {
    let mut generics = base.clone();
    let lt_param: syn::LifetimeParam = syn::parse_quote! { '__ir };
    let param: syn::TypeParam = syn::parse_quote! { __CallSemI };
    generics
        .params
        .insert(0, syn::GenericParam::Lifetime(lt_param));
    generics.params.push(syn::GenericParam::Type(param));
    generics
}
