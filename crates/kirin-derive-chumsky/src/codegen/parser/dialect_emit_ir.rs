//! Code generation for the `HasDialectEmitIR` trait implementation.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use kirin_derive_toolkit::codegen::combine_where_clauses;

use super::super::{collect_all_value_types_needing_bounds, collect_wrapper_types};
use super::GenerateHasDialectParser;

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

    /// Generates `HasDialectEmitIR` for a regular (non-wrapper) type.
    ///
    /// The impl carries all bounds needed by the AST helper's local dialect
    /// emission at the impl level, allowing wrapper enums to compose existing
    /// dialects without requiring every transitive inner statement to be
    /// convertible directly into the outer language.
    ///
    /// Uses a single lifetime `'tokens` (with `HasDialectParser<'tokens>`)
    /// for HRTB compatibility — see the trait docs for rationale.
    fn generate_regular_dialect_emit_ir_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_type = &ir_input.attrs.ir_type;
        let ir_path = &self.config.ir_path;

        // Build impl generics: <'tokens, T..., Language, LanguageOutput>
        // Uses only 'tokens (no 'src) for HRTB compatibility.
        let mut impl_generics = ir_input.generics.clone();
        let tokens_lt = syn::Lifetime::new("'tokens", proc_macro2::Span::call_site());
        if !impl_generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"))
        {
            impl_generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt)),
            );
        }
        impl_generics.params.push(syn::parse_quote! { Language });
        impl_generics
            .params
            .push(syn::parse_quote! { LanguageOutput });

        let (impl_g, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        // Collect bounds
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds: Vec<syn::WherePredicate> = value_types
            .iter()
            .flat_map(|ty| {
                [
                    syn::parse_quote! {
                        #ty: #crate_path::HasParser<'tokens> + 'tokens
                    },
                    syn::parse_quote! {
                        <#ty as #crate_path::HasParser<'tokens>>::Output:
                            #crate_path::EmitIR<Language, Output = #ty>
                    },
                ]
            })
            .collect();
        let wrapper_types = collect_wrapper_types(ir_input);

        // Build where clause.
        // Placeholder is needed for ResultValue fields (auto-default) AND for
        // wrapper types (wrapped dialects may themselves have ResultValue fields
        // whose Placeholder bound is satisfied through HasDialectEmitIR).
        let needs_placeholder =
            crate::codegen::has_result_fields(ir_input) || !wrapper_types.is_empty();

        // Wrapper types need HasDialectEmitIR<'tokens, Language, LanguageOutput>
        // bounds so recursive statement emission crosses the boundary through
        // a nominal witness trait instead of an associated-type projection.
        let wrapper_emit_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| {
                syn::parse_quote! {
                    #ty: #crate_path::HasDialectEmitIR<'tokens, Language, LanguageOutput>
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

        // Base bounds — use 'tokens for both lifetime positions
        let base: syn::WherePredicate =
            syn::parse_quote! { Language: #ir_path::Dialect<Type = #ir_type> };
        wc.predicates.push(base);
        wc.predicates
            .push(syn::parse_quote! { LanguageOutput: Clone + PartialEq + 'tokens });

        let ir_type_bound: syn::WherePredicate = syn::parse_quote! {
            #ir_type: #crate_path::HasParser<'tokens> + 'tokens
        };
        wc.predicates.push(ir_type_bound);

        let ir_output_bound: syn::WherePredicate = syn::parse_quote! {
            <#ir_type as #crate_path::HasParser<'tokens>>::Output:
                #crate_path::EmitIR<Language, Output = <Language as #ir_path::Dialect>::Type>
        };
        wc.predicates.push(ir_output_bound);

        wc.predicates.extend(value_type_bounds);
        wc.predicates.extend(wrapper_emit_bounds);

        // Add placeholder bound as a predicate if needed
        if needs_placeholder {
            let placeholder: syn::WherePredicate = syn::parse_quote! {
                #ir_type: #ir_path::Placeholder
            };
            wc.predicates.push(placeholder);
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

    /// Generates `HasDialectEmitIR` for a wrapper struct that delegates to
    /// the wrapped type.
    ///
    /// Uses a single lifetime `'tokens` for HRTB compatibility.
    fn generate_wrapper_struct_dialect_emit_ir_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;
        let ir_path = &self.config.ir_path;

        // Single lifetime 'tokens (no 'src) for HRTB compatibility.
        let mut impl_generics = ir_input.generics.clone();
        let tokens_lt = syn::Lifetime::new("'tokens", proc_macro2::Span::call_site());
        if !impl_generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"))
        {
            impl_generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt)),
            );
        }
        impl_generics.params.push(syn::parse_quote! { Language });
        impl_generics
            .params
            .push(syn::parse_quote! { LanguageOutput });

        let (impl_g, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        let mut wc = match combined_where {
            Some(wc) => wc,
            None => syn::WhereClause {
                where_token: syn::token::Where::default(),
                predicates: syn::punctuated::Punctuated::new(),
            },
        };

        // Only need HasDialectEmitIR on the wrapped type + Language: Dialect
        let wrapped_bound: syn::WherePredicate = syn::parse_quote! {
            #wrapped_ty: #crate_path::HasDialectEmitIR<'tokens, Language, LanguageOutput>
        };
        let language_bound: syn::WherePredicate = syn::parse_quote! {
            Language: #ir_path::Dialect
        };
        let language_output_bound: syn::WherePredicate =
            syn::parse_quote! { LanguageOutput: Clone + PartialEq + 'tokens };
        wc.predicates.push(wrapped_bound);
        wc.predicates.push(language_bound);
        wc.predicates.push(language_output_bound);

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
