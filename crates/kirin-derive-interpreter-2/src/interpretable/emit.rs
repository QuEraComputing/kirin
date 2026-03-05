use super::DeriveInterpretable;
use kirin_derive_toolkit::codegen::combine_where_clauses;
use kirin_derive_toolkit::emit::Emit;
use kirin_derive_toolkit::ir::{self, StandardLayout};
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::tokens::{MatchArm, MatchExpr, Method, TraitImpl};
use quote::quote;

impl<'ir> Emit<'ir, StandardLayout> for DeriveInterpretable {
    fn emit_struct(
        &mut self,
        data: &'ir ir::DataStruct<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let info = self.statement_info(&data.0)?;
        let interp_crate = self.interpreter_crate_path();
        let type_name = &input.core.name;

        if !info.is_wrapper {
            return Err(darling::Error::custom(format!(
                "Cannot derive `Interpretable` for struct `{}` without `#[wraps]`. \
                 Either implement `Interpretable` manually, or wrap an inner type with `#[wraps]`.",
                type_name
            ))
            .with_span(&data.0.name));
        }

        let wrapper_ty = info.wrapper_ty.as_ref().unwrap();
        let pattern = &info.pattern;
        let binding = info.wrapper_binding.as_ref().unwrap();

        let generics = add_interpreter_params(&input.core.generics);
        let (_, _, orig_where) = input.core.generics.split_for_impl();
        let extra_where: syn::WhereClause = syn::parse_quote! {
            where
                __InterpI: #interp_crate::Interpreter<'__ir>,
                __InterpL: ::kirin_ir::Dialect,
                #wrapper_ty: #interp_crate::Interpretable<'__ir, __InterpI, __InterpL>
        };

        let trait_impl = TraitImpl::new(
            generics,
            quote! { #interp_crate::Interpretable<'__ir, __InterpI, __InterpL> },
            type_name,
        )
        .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
        .method(Method {
            name: syn::parse_quote! { interpret },
            self_arg: quote! { &self },
            params: vec![quote! { interpreter: &mut __InterpI }],
            return_type: Some(
                quote! { Result<#interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error> },
            ),
            body: quote! {
                let Self #pattern = self;
                #binding.interpret(interpreter)
            },
        });

        Ok(quote! { #trait_impl })
    }

    fn emit_enum(
        &mut self,
        data: &'ir ir::DataEnum<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let interp_crate = self.interpreter_crate_path();
        let type_name = &input.core.name;

        // Check that ALL variants are wrappers
        let mut non_wrapper_variants = Vec::new();
        for variant in &data.variants {
            let info = self.statement_info(variant)?;
            if !info.is_wrapper {
                non_wrapper_variants.push(info.name.clone());
            }
        }

        if !non_wrapper_variants.is_empty() {
            let names: Vec<String> = non_wrapper_variants.iter().map(|n| n.to_string()).collect();
            return Err(darling::Error::custom(format!(
                "Cannot derive `Interpretable` for enum `{}`: variant(s) {} are not `#[wraps]`. \
                 Either implement `Interpretable` manually, or split each variant as a separate \
                 struct, impl `Interpretable` on it, then wrap it with `#[wraps]`.",
                type_name,
                names.join(", "),
            ))
            .with_span(&type_name));
        }

        let mut wrapper_types: Vec<&syn::Type> = Vec::new();
        let mut match_arms = Vec::new();

        for variant in &data.variants {
            let info = self.statement_info(variant)?;
            let variant_name = &info.name;
            let pattern = &info.pattern;
            let wrapper_ty = info.wrapper_ty.as_ref().unwrap();
            wrapper_types.push(wrapper_ty);
            let binding = info.wrapper_binding.as_ref().unwrap();

            match_arms.push(MatchArm {
                pattern: quote! { Self::#variant_name #pattern },
                guard: None,
                body: quote! { #binding.interpret(interpreter) },
            });
        }

        let where_bounds: Vec<proc_macro2::TokenStream> = wrapper_types
            .iter()
            .map(|ty| {
                quote! { #ty: #interp_crate::Interpretable<'__ir, __InterpI, __InterpL> }
            })
            .collect();

        let generics = add_interpreter_params(&input.core.generics);
        let (_, _, orig_where) = input.core.generics.split_for_impl();
        let extra_where: syn::WhereClause = syn::parse_quote! {
            where
                __InterpI: #interp_crate::Interpreter<'__ir>,
                __InterpL: ::kirin_ir::Dialect,
                #(#where_bounds),*
        };

        let match_expr = MatchExpr {
            subject: quote! { self },
            arms: match_arms,
        };

        let trait_impl = TraitImpl::new(
            generics,
            quote! { #interp_crate::Interpretable<'__ir, __InterpI, __InterpL> },
            type_name,
        )
        .where_clause(combine_where_clauses(Some(&extra_where), orig_where))
        .method(Method {
            name: syn::parse_quote! { interpret },
            self_arg: quote! { &self },
            params: vec![quote! { interpreter: &mut __InterpI }],
            return_type: Some(
                quote! { Result<#interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error> },
            ),
            body: quote! { #match_expr },
        });

        Ok(quote! { #trait_impl })
    }
}

fn add_interpreter_params(base: &syn::Generics) -> syn::Generics {
    let mut generics = base.clone();
    let lt_param: syn::LifetimeParam = syn::parse_quote! { '__ir };
    let interp_param: syn::TypeParam = syn::parse_quote! { __InterpI };
    let lang_param: syn::TypeParam = syn::parse_quote! { __InterpL };
    generics
        .params
        .insert(0, syn::GenericParam::Lifetime(lt_param));
    generics.params.push(syn::GenericParam::Type(interp_param));
    generics.params.push(syn::GenericParam::Type(lang_param));
    generics
}
