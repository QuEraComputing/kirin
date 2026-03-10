use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use kirin_derive_toolkit::codegen::combine_where_clauses;

use super::super::{collect_all_value_types_needing_bounds, collect_wrapper_types};
use super::GenerateHasDialectParser;

impl GenerateHasDialectParser {
    /// Generates the top-level IR emission witness for parsed dialect output.
    pub(super) fn generate_has_parser_emit_ir_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        if let kirin_derive_toolkit::ir::Data::Struct(data) = &ir_input.data
            && let Some(wrapper) = &data.0.wraps
        {
            return self
                .generate_wrapper_struct_has_parser_emit_ir_impl(ir_input, wrapper, crate_path);
        }

        self.generate_regular_has_parser_emit_ir_impl(ir_input, ast_name, crate_path)
    }

    fn generate_regular_has_parser_emit_ir_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_path = &self.config.ir_path;
        let ir_type = &ir_input.attrs.ir_type;

        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        let ast_self_name = syn::Ident::new(&format!("{}Self", ast_name), ast_name.span());
        let ast_self_type = self.build_ast_self_type_reference(ir_input, &ast_self_name, ir_type);
        let language = quote! { #original_name #ty_generics };

        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds: Vec<syn::WherePredicate> = value_types
            .iter()
            .flat_map(|ty| {
                [
                    syn::parse_quote! {
                        #ty: #crate_path::HasParser<'t> + 't
                    },
                    syn::parse_quote! {
                        <#ty as #crate_path::HasParser<'t>>::Output:
                            #crate_path::EmitIR<#language, Output = #ty>
                    },
                ]
            })
            .collect();

        let wrapper_types = collect_wrapper_types(ir_input);
        let wrapper_emit_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| {
                syn::parse_quote! {
                    #ty: #crate_path::HasDialectEmitIR<'t, #language, #ast_self_type>
                }
            })
            .collect();

        let mut wc = match combined_where {
            Some(wc) => wc,
            None => syn::WhereClause {
                where_token: syn::token::Where::default(),
                predicates: syn::punctuated::Punctuated::new(),
            },
        };

        wc.predicates
            .push(syn::parse_quote! { #language: #ir_path::Dialect<Type = #ir_type> });
        wc.predicates
            .push(syn::parse_quote! { #ir_type: #crate_path::HasParser<'t> + 't });
        wc.predicates.push(syn::parse_quote! {
            <#ir_type as #crate_path::HasParser<'t>>::Output:
                #crate_path::EmitIR<#language, Output = <#language as #ir_path::Dialect>::Type>
        });
        wc.predicates.extend(value_type_bounds);
        wc.predicates.extend(wrapper_emit_bounds);

        if self.needs_result_fields(ir_input) || !wrapper_types.is_empty() {
            wc.predicates.push(syn::parse_quote! {
                <#language as #ir_path::Dialect>::Type: #ir_path::Placeholder
            });
        }

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasParserEmitIR<'t> for #original_name #ty_generics
            #wc
            {
                fn emit_parsed(
                    output: &<Self as #crate_path::HasParser<'t>>::Output,
                    ctx: &mut #crate_path::EmitContext<'_, Self>,
                ) -> ::core::result::Result<#ir_path::Statement, #crate_path::EmitError> {
                    let dialect_variant = output.0.emit_with(
                        ctx,
                        &|stmt, ctx| {
                            <#original_name #ty_generics as #crate_path::HasParserEmitIR<'t>>::emit_parsed(stmt, ctx)
                        },
                    )?;
                    Ok(ctx.stage.statement().definition(dialect_variant).new())
                }
            }
        }
    }

    fn generate_wrapper_struct_has_parser_emit_ir_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;
        let ir_path = &self.config.ir_path;
        let ir_type = &ir_input.attrs.ir_type;

        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        let language = quote! { #original_name #ty_generics };
        let emit_bound: syn::WherePredicate = syn::parse_quote! {
            <#wrapped_ty as #crate_path::HasParser<'t>>::Output:
                #crate_path::EmitIR<#language, Output = #ir_path::Statement>
        };
        let wrapped_bound: syn::WherePredicate =
            syn::parse_quote! { #wrapped_ty: #crate_path::HasParser<'t> };

        let where_clause = match combined_where {
            Some(mut wc) => {
                wc.predicates
                    .push(syn::parse_quote! { #language: #ir_path::Dialect });
                wc.predicates.push(wrapped_bound);
                wc.predicates.push(emit_bound);
                quote! { #wc }
            }
            None => {
                quote! {
                    where
                        #language: #ir_path::Dialect<Type = #ir_type>,
                        #wrapped_ty: #crate_path::HasParser<'t>,
                        #emit_bound
                }
            }
        };

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasParserEmitIR<'t> for #original_name #ty_generics
            #where_clause
            {
                fn emit_parsed(
                    output: &<Self as #crate_path::HasParser<'t>>::Output,
                    ctx: &mut #crate_path::EmitContext<'_, Self>,
                ) -> ::core::result::Result<#ir_path::Statement, #crate_path::EmitError> {
                    <#wrapped_ty as #crate_path::HasParser<'t>>::Output::emit(output, ctx)
                }
            }
        }
    }
}
