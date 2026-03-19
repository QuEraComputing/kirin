//! Code generation for the `ParseEmit` and `HasParserEmitIR` trait implementations.
//!
//! `HasParserEmitIR<'t>` carries a concrete lifetime so the compiler can
//! normalize `<Self as HasParser<'t>>::Output` to the concrete AST type.
//! `ParseEmit` is the public, lifetime-free trait that delegates to
//! `HasParserEmitIR` internally via `parse_ast`.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::codegen::{ImplBounds, init_where_clause};

use super::GenerateHasDialectParser;

impl GenerateHasDialectParser {
    /// Generates both `HasParserEmitIR` and `ParseEmit` impls.
    pub(super) fn generate_parse_emit_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        if let kirin_derive_toolkit::ir::Data::Struct(data) = &ir_input.data
            && let Some(wrapper) = &data.0.wraps
        {
            return self.generate_wrapper_struct_parse_emit_impls(ir_input, wrapper, crate_path);
        }

        self.generate_regular_parse_emit_impls(ir_input, ast_name, crate_path)
    }

    fn generate_regular_parse_emit_impls(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_path = &self.config.ir_path;
        let ir_type = &ir_input.attrs.ir_type;

        // --- HasParserEmitIR<'t> impl (same as old generate_has_parser_emit_ir_impl) ---
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics_tok, _, impl_where_clause) = impl_generics.split_for_impl();
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
            wc.predicates
                .extend(bounds.placeholder_predicates(&language));
        }

        let has_parser_emit_ir_impl = quote! {
            #[automatically_derived]
            impl #impl_generics_tok #crate_path::HasParserEmitIR<'t> for #original_name #ty_generics
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
        };

        // --- ParseEmit impl (delegates to HasParserEmitIR) ---
        let parse_emit_impl = self.generate_parse_emit_delegating_impl(ir_input, crate_path);

        quote! {
            #has_parser_emit_ir_impl
            #parse_emit_impl
        }
    }

    fn generate_wrapper_struct_parse_emit_impls(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;
        let ir_path = &self.config.ir_path;
        let ir_type = &ir_input.attrs.ir_type;

        // --- HasParserEmitIR<'t> impl for wrapper struct ---
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics_tok, _, impl_where_clause) = impl_generics.split_for_impl();
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

        let has_parser_emit_ir_impl = quote! {
            #[automatically_derived]
            impl #impl_generics_tok #crate_path::HasParserEmitIR<'t> for #original_name #ty_generics
            #wc
            {
                fn emit_parsed(
                    output: &<Self as #crate_path::HasParser<'t>>::Output,
                    ctx: &mut #crate_path::EmitContext<'_, Self>,
                ) -> ::core::result::Result<#ir_path::Statement, #crate_path::EmitError> {
                    <#wrapped_ty as #crate_path::HasParser<'t>>::Output::emit(output, ctx)
                }
            }
        };

        // --- ParseEmit impl (delegates to HasParserEmitIR) ---
        let parse_emit_impl = self.generate_parse_emit_delegating_impl(ir_input, crate_path);

        quote! {
            #has_parser_emit_ir_impl
            #parse_emit_impl
        }
    }

    /// Generates a `ParseEmit` impl that delegates to `HasParserEmitIR`.
    ///
    /// This is shared between regular and wrapper struct paths.
    fn generate_parse_emit_delegating_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_path = &self.config.ir_path;

        let (impl_generics, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let mut wc = init_where_clause(where_clause, None);
        wc.predicates.push(syn::parse_quote! {
            for<'t> Self: #crate_path::HasParserEmitIR<'t>
        });

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::ParseEmit for #original_name #ty_generics
            #wc
            {
                fn parse_and_emit(
                    input: &str,
                    ctx: &mut #crate_path::EmitContext<'_, Self>,
                ) -> ::core::result::Result<#ir_path::Statement, #crate_path::ChumskyError> {
                    let ast = #crate_path::parse_ast::<Self>(input)?;
                    #crate_path::HasParserEmitIR::emit_parsed(&ast, ctx).map_err(#crate_path::ChumskyError::Emit)
                }
            }
        }
    }
}
