//! Code generation for the `HasParserEmitIR` trait implementation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::codegen::{ImplBounds, init_where_clause};

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

        let ast_self_name = syn::Ident::new(&format!("{}Self", ast_name), ast_name.span());
        let ast_self_type = self.build_ast_self_type_reference(ir_input, &ast_self_name, ir_type);
        let language = quote! { #original_name #ty_generics };

        let lt_t: syn::Lifetime = syn::parse_quote!('t);
        let bounds = ImplBounds::from_input(ir_input, &self.config);

        let mut wc = init_where_clause(where_clause, impl_where_clause);
        wc.predicates
            .push(syn::parse_quote! { #language: #ir_path::Dialect<Type = #ir_type> });
        wc.predicates.push(bounds.ir_type_has_parser(&lt_t));
        wc.predicates.push(bounds.ir_type_emit_ir(&lt_t, &language));
        wc.predicates
            .extend(bounds.value_types_all(&lt_t, &language));
        wc.predicates
            .extend(bounds.wrappers_emit_ir(&lt_t, &language, &ast_self_type));
        if bounds.needs_placeholder_with_wrappers() {
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

        let language = quote! { #original_name #ty_generics };

        let mut wc = init_where_clause(where_clause, impl_where_clause);
        wc.predicates
            .push(syn::parse_quote! { #language: #ir_path::Dialect<Type = #ir_type> });
        wc.predicates
            .push(syn::parse_quote! { #wrapped_ty: #crate_path::HasParser<'t> });
        wc.predicates.push(syn::parse_quote! {
            <#wrapped_ty as #crate_path::HasParser<'t>>::Output:
                #crate_path::EmitIR<#language, Output = #ir_path::Statement>
        });

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasParserEmitIR<'t> for #original_name #ty_generics
            #wc
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
