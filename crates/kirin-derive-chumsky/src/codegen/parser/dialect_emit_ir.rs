//! Code generation for the `HasDialectEmitIR` trait implementation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::codegen::{ImplBounds, init_where_clause};

use super::GenerateHasDialectParser;

/// Inserts `'tokens`, `Language`, and `LanguageOutput` type parameters into
/// a clone of the input's generics for `HasDialectEmitIR` impls.
fn build_dialect_emit_ir_generics(
    ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
) -> syn::Generics {
    let mut generics = ir_input.generics.clone();
    let tokens_lt = syn::Lifetime::new("'tokens", proc_macro2::Span::call_site());
    if !generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"))
    {
        generics.params.insert(
            0,
            syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt)),
        );
    }
    generics.params.push(syn::parse_quote! { Language });
    generics.params.push(syn::parse_quote! { LanguageOutput });
    generics
}

impl GenerateHasDialectParser {
    /// Generates the `HasDialectEmitIR` impl for the original dialect type.
    pub(super) fn generate_has_dialect_emit_ir_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        crate_path: &syn::Path,
    ) -> TokenStream {
        if let kirin_derive_toolkit::ir::Data::Struct(data) = &ir_input.data
            && let Some(wrapper) = &data.0.wraps
        {
            return self
                .generate_wrapper_struct_dialect_emit_ir_impl(ir_input, wrapper, crate_path);
        }

        self.generate_regular_dialect_emit_ir_impl(ir_input, crate_path)
    }

    fn generate_regular_dialect_emit_ir_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_path = &self.config.ir_path;
        let ir_type = &ir_input.attrs.ir_type;

        let impl_generics = build_dialect_emit_ir_generics(ir_input);
        let (impl_g, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let lt_tokens: syn::Lifetime = syn::parse_quote!('tokens);
        let lang = quote! { Language };
        let lang_output = quote! { LanguageOutput };
        let bounds = ImplBounds::from_input(ir_input, &self.config);

        let mut wc = init_where_clause(where_clause, impl_where_clause);
        wc.predicates
            .push(syn::parse_quote! { Language: #ir_path::Dialect<Type = #ir_type> });
        wc.predicates
            .push(syn::parse_quote! { LanguageOutput: Clone + PartialEq + 'tokens });
        wc.predicates.push(bounds.ir_type_has_parser(&lt_tokens));
        wc.predicates
            .push(bounds.ir_type_emit_ir(&lt_tokens, &lang));
        wc.predicates
            .extend(bounds.value_types_all(&lt_tokens, &lang));
        wc.predicates
            .extend(bounds.wrappers_emit_ir(&lt_tokens, &lang, &lang_output));
        if bounds.needs_placeholder_with_wrappers() {
            wc.predicates
                .push(syn::parse_quote! { #ir_type: #ir_path::Placeholder });
        }

        quote! {
            #[automatically_derived]
            impl #impl_g #crate_path::HasDialectEmitIR<'tokens, Language, LanguageOutput>
                for #original_name #ty_generics
            #wc
            {
                #[inline]
                fn emit_output<__TypeOutput, __EmitLanguageOutput>(
                    output: &<Self as #crate_path::HasDialectParser<'tokens>>::Output<__TypeOutput, LanguageOutput>,
                    ctx: &mut #crate_path::EmitContext<'_, Language>,
                    emit_language_output: &__EmitLanguageOutput,
                ) -> ::core::result::Result<Self, #crate_path::EmitError>
                where
                    __TypeOutput: Clone + PartialEq + 'tokens,
                    __EmitLanguageOutput: for<'ctx> Fn(
                        &LanguageOutput,
                        &mut #crate_path::EmitContext<'ctx, Language>,
                    ) -> ::core::result::Result<#ir_path::Statement, #crate_path::EmitError>,
                {
                    output.emit_with(ctx, emit_language_output)
                }
            }
        }
    }

    fn generate_wrapper_struct_dialect_emit_ir_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;
        let ir_path = &self.config.ir_path;

        let impl_generics = build_dialect_emit_ir_generics(ir_input);
        let (impl_g, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let mut wc = init_where_clause(where_clause, impl_where_clause);
        wc.predicates
            .push(syn::parse_quote! { #wrapped_ty: #crate_path::HasDialectEmitIR<'tokens, Language, LanguageOutput> });
        wc.predicates
            .push(syn::parse_quote! { Language: #ir_path::Dialect });
        wc.predicates
            .push(syn::parse_quote! { LanguageOutput: Clone + PartialEq + 'tokens });

        quote! {
            #[automatically_derived]
            impl #impl_g #crate_path::HasDialectEmitIR<'tokens, Language, LanguageOutput>
                for #original_name #ty_generics
            #wc
            {
                #[inline]
                fn emit_output<__TypeOutput, __EmitLanguageOutput>(
                    output: &<Self as #crate_path::HasDialectParser<'tokens>>::Output<__TypeOutput, LanguageOutput>,
                    ctx: &mut #crate_path::EmitContext<'_, Language>,
                    emit_language_output: &__EmitLanguageOutput,
                ) -> ::core::result::Result<Self, #crate_path::EmitError>
                where
                    __TypeOutput: Clone + PartialEq + 'tokens,
                    __EmitLanguageOutput: for<'ctx> Fn(
                        &LanguageOutput,
                        &mut #crate_path::EmitContext<'ctx, Language>,
                    ) -> ::core::result::Result<#ir_path::Statement, #crate_path::EmitError>,
                {
                    <#wrapped_ty as #crate_path::HasDialectEmitIR<'tokens, Language, LanguageOutput>>::emit_output(
                        output,
                        ctx,
                        emit_language_output,
                    ).map(Into::into)
                }
            }
        }
    }
}
