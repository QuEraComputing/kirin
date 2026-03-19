use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::codegen::{ImplBounds, WhereClauseExt};

use super::GenerateEmitIR;

impl GenerateEmitIR {
    pub(super) fn generate_ast_self_emit_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        _ast_name: &syn::Ident,
        ast_self_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_path = &self.config.ir_path;
        let (_, original_ty_generics, _) = ir_input.generics.split_for_impl();

        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        let impl_generics = if type_params.is_empty() {
            quote! { <'t, TypeOutput, Language> }
        } else {
            let type_param_bounds: Vec<_> = ir_input
                .generics
                .type_params()
                .map(|p| {
                    let ident = &p.ident;
                    let bounds = &p.bounds;
                    if bounds.is_empty() {
                        quote! { #ident }
                    } else {
                        quote! { #ident: #bounds }
                    }
                })
                .collect();
            quote! { <'t, #(#type_param_bounds,)* TypeOutput, Language> }
        };

        let ast_self_ty_generics = if type_params.is_empty() {
            quote! { <'t, TypeOutput> }
        } else {
            quote! { <'t, #(#type_params,)* TypeOutput> }
        };

        let lt_t: syn::Lifetime = syn::parse_quote!('t);
        let lang = quote! { Language };
        let bounds = ImplBounds::from_input(ir_input, &self.config);

        let mut wc = syn::WhereClause {
            where_token: syn::token::Where::default(),
            predicates: syn::punctuated::Punctuated::new(),
        };
        wc.predicates.push(syn::parse_quote! {
            Language: #ir_path::Dialect + From<#original_name #original_ty_generics>
        });
        wc.push_opt(bounds.dialect_type_bound(&lang));
        if bounds.needs_placeholder() {
            wc.predicates.extend(bounds.placeholder_predicates(&lang));
        }
        wc.predicates
            .push(syn::parse_quote! { TypeOutput: Clone + PartialEq });
        wc.predicates.push(bounds.ir_type_has_parser(&lt_t));
        wc.predicates.push(bounds.ir_type_emit_ir(&lt_t, &lang));
        wc.predicates.extend(bounds.value_types_all(&lt_t, &lang));
        wc.predicates.extend(bounds.wrappers_emit_ir(
            &lt_t,
            &lang,
            &quote! { #ast_self_name #ast_self_ty_generics },
        ));

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::EmitIR<Language> for #ast_self_name #ast_self_ty_generics
            #wc
            {
                type Output = #ir_path::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> ::core::result::Result<Self::Output, #crate_path::EmitError> {
                    let dialect_variant = self.0.emit_with(
                        ctx,
                        &|stmt, ctx| {
                            <#ast_self_name #ast_self_ty_generics as #crate_path::EmitIR<Language>>::emit(stmt, ctx)
                        },
                    )?;
                    Ok(ctx.stage.statement(dialect_variant))
                }
            }
        }
    }
}
