//! Trait implementation generation for HasParser and HasDialectParser.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use kirin_derive_toolkit::codegen::GenericsBuilder;

use super::GenerateHasDialectParser;
use crate::codegen::{ImplBounds, init_where_clause};

impl GenerateHasDialectParser {
    /// Generates the `HasParser` impl for the original type.
    pub(super) fn generate_has_parser_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        if let kirin_derive_toolkit::ir::Data::Struct(data) = &ir_input.data
            && let Some(wrapper) = &data.0.wraps
        {
            return self.generate_wrapper_struct_has_parser_impl(ir_input, wrapper, crate_path);
        }

        let original_name = &ir_input.name;
        let ir_type = &ir_input.attrs.ir_type;

        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let lt_t: syn::Lifetime = syn::parse_quote!('t);
        let bounds = ImplBounds::from_input(ir_input, &self.config);
        let mut wc = init_where_clause(where_clause, impl_where_clause);
        wc.predicates.push(bounds.ir_type_has_parser(&lt_t));
        wc.predicates.extend(bounds.value_types_has_parser(&lt_t));
        wc.predicates
            .extend(bounds.wrappers_has_dialect_parser(&lt_t));

        let ast_self_name = syn::Ident::new(&format!("{}Self", ast_name), ast_name.span());
        let ast_self_type = self.build_ast_self_type_reference(ir_input, &ast_self_name, ir_type);
        let type_output = quote! { <#ir_type as #crate_path::HasParser<'t>>::Output };

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasParser<'t> for #original_name #ty_generics
            #wc
            {
                type Output = #ast_self_type;

                fn parser<I>() -> #crate_path::BoxedParser<'t, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'t>,
                {
                    use #crate_path::chumsky::prelude::*;
                    #crate_path::chumsky::recursive::recursive(|language| {
                        <#original_name #ty_generics as #crate_path::HasDialectParser<
                            't,
                        >>::recursive_parser::<I, #type_output, Self::Output>(language)
                            .map(|inner| #ast_self_name::new(inner))
                    }).boxed()
                }
            }
        }
    }

    fn generate_wrapper_struct_has_parser_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;

        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let mut wc = init_where_clause(where_clause, impl_where_clause);
        wc.predicates
            .push(syn::parse_quote! { #wrapped_ty: #crate_path::HasParser<'t> });

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasParser<'t> for #original_name #ty_generics
            #wc
            {
                type Output = <#wrapped_ty as #crate_path::HasParser<'t>>::Output;

                fn parser<I>() -> #crate_path::BoxedParser<'t, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'t>,
                {
                    <#wrapped_ty as #crate_path::HasParser<'t>>::parser()
                }
            }
        }
    }

    pub(super) fn generate_dialect_parser_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        if let kirin_derive_toolkit::ir::Data::Struct(data) = &ir_input.data
            && let Some(wrapper) = &data.0.wraps
        {
            return self.generate_wrapper_struct_dialect_parser_impl(ir_input, wrapper, crate_path);
        }

        let original_name = &ir_input.name;

        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let lt_t: syn::Lifetime = syn::parse_quote!('t);
        let bounds = ImplBounds::from_input(ir_input, &self.config);
        let mut wc = init_where_clause(where_clause, impl_where_clause);
        wc.predicates.push(bounds.ir_type_has_parser(&lt_t));
        wc.predicates.extend(bounds.value_types_has_parser(&lt_t));
        wc.predicates
            .extend(bounds.wrappers_has_dialect_parser(&lt_t));

        let parser_body = self.generate_dialect_parser_body(ir_input, ast_name, crate_path);
        let ast_type = self.build_ast_type_with_type_params(ir_input, ast_name);

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasDialectParser<'t>
                for #original_name #ty_generics
            #wc
            {
                type Output<__TypeOutput, __LanguageOutput> = #ast_type
                where
                    __TypeOutput: Clone + PartialEq + 't,
                    __LanguageOutput: Clone + PartialEq + 't;

                #[inline]
                fn namespaced_parser<I, __TypeOutput, __LanguageOutput>(
                    language: #crate_path::RecursiveParser<'t, I, __LanguageOutput>,
                    namespace: &[&'static str],
                ) -> #crate_path::BoxedParser<'t, I, Self::Output<__TypeOutput, __LanguageOutput>>
                where
                    I: #crate_path::TokenInput<'t>,
                    __TypeOutput: Clone + PartialEq + 't,
                    __LanguageOutput: Clone + PartialEq + 't,
                {
                    use #crate_path::chumsky::prelude::*;
                    #parser_body.boxed()
                }
            }
        }
    }

    /// Builds impl generics for the original type's HasDialectParser impl.
    pub(super) fn build_original_type_impl_generics(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.config.ir_path).with_lifetimes(&ir_input.generics)
    }

    fn build_ast_type_with_type_params(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let type_params: Vec<_> = ir_input
            .generics
            .params
            .iter()
            .filter_map(|p| {
                if let syn::GenericParam::Type(tp) = p {
                    let ident = &tp.ident;
                    Some(quote! { #ident })
                } else {
                    None
                }
            })
            .collect();

        if type_params.is_empty() {
            quote! { #ast_name<'t, __TypeOutput, __LanguageOutput> }
        } else {
            quote! { #ast_name<'t, #(#type_params,)* __TypeOutput, __LanguageOutput> }
        }
    }

    pub(super) fn build_ast_self_type_reference(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_self_name: &syn::Ident,
        ir_type: &syn::Path,
    ) -> TokenStream {
        let crate_path = &self.config.crate_path;

        let type_params: Vec<_> = ir_input
            .generics
            .params
            .iter()
            .filter_map(|p| {
                if let syn::GenericParam::Type(tp) = p {
                    let ident = &tp.ident;
                    Some(quote! { #ident })
                } else {
                    None
                }
            })
            .collect();

        let type_output = quote! { <#ir_type as #crate_path::HasParser<'t>>::Output };

        if type_params.is_empty() {
            quote! { #ast_self_name<'t, #type_output> }
        } else {
            quote! { #ast_self_name<'t, #(#type_params,)* #type_output> }
        }
    }

    pub(super) fn build_ast_type_reference(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        type_output: &TokenStream,
        language_output: &TokenStream,
    ) -> TokenStream {
        let type_params: Vec<_> = ir_input
            .generics
            .params
            .iter()
            .filter_map(|p| {
                if let syn::GenericParam::Type(tp) = p {
                    let ident = &tp.ident;
                    Some(quote! { #ident })
                } else {
                    None
                }
            })
            .collect();

        if type_params.is_empty() {
            quote! { #ast_name<'t, #type_output, #language_output> }
        } else {
            quote! { #ast_name<'t, #(#type_params,)* #type_output, #language_output> }
        }
    }

    fn generate_wrapper_struct_dialect_parser_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;

        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let mut wc = init_where_clause(where_clause, impl_where_clause);
        wc.predicates
            .push(syn::parse_quote! { #wrapped_ty: #crate_path::HasDialectParser<'t> });

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasDialectParser<'t>
                for #original_name #ty_generics
            #wc
            {
                type Output<__TypeOutput, __LanguageOutput> =
                    <#wrapped_ty as #crate_path::HasDialectParser<'t>>::Output<__TypeOutput, __LanguageOutput>
                where
                    __TypeOutput: Clone + PartialEq + 't,
                    __LanguageOutput: Clone + PartialEq + 't;

                #[inline]
                fn namespaced_parser<I, __TypeOutput, __LanguageOutput>(
                    language: #crate_path::RecursiveParser<'t, I, __LanguageOutput>,
                    namespace: &[&'static str],
                ) -> #crate_path::BoxedParser<'t, I, Self::Output<__TypeOutput, __LanguageOutput>>
                where
                    I: #crate_path::TokenInput<'t>,
                    __TypeOutput: Clone + PartialEq + 't,
                    __LanguageOutput: Clone + PartialEq + 't,
                {
                    <#wrapped_ty as #crate_path::HasDialectParser<'t>>::namespaced_parser::<I, __TypeOutput, __LanguageOutput>(language, namespace)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use kirin_derive_toolkit::codegen::GenericsBuilder;
    use quote::quote;

    fn format_generics(generics: &syn::Generics) -> String {
        let tokens = quote! { #generics };
        tokens.to_string()
    }

    #[test]
    fn test_with_lifetimes_empty() {
        let ir_path: syn::Path = syn::parse_quote!(::kirin::ir);
        let builder = GenericsBuilder::new(&ir_path);

        let base = syn::Generics::default();
        let result = builder.with_lifetimes(&base);

        insta::assert_snapshot!("with_lifetimes_empty", format_generics(&result));
    }

    #[test]
    fn test_with_lifetimes_existing_type_param() {
        let ir_path: syn::Path = syn::parse_quote!(::kirin::ir);
        let builder = GenericsBuilder::new(&ir_path);

        let base: syn::Generics = syn::parse_quote!(<T: Clone>);
        let result = builder.with_lifetimes(&base);

        insta::assert_snapshot!("with_lifetimes_existing_type", format_generics(&result));
    }

    #[test]
    fn test_with_language_empty() {
        let ir_path: syn::Path = syn::parse_quote!(::kirin::ir);
        let builder = GenericsBuilder::new(&ir_path);

        let base = syn::Generics::default();
        let result = builder.with_language(&base);

        insta::assert_snapshot!("with_language_empty", format_generics(&result));
    }

    #[test]
    fn test_with_language_custom_ir_path() {
        let ir_path: syn::Path = syn::parse_quote!(my_kirin);
        let builder = GenericsBuilder::new(&ir_path);

        let base = syn::Generics::default();
        let result = builder.with_language(&base);

        insta::assert_snapshot!("with_language_custom_ir", format_generics(&result));
    }

    #[test]
    fn test_with_language_existing_type_param() {
        let ir_path: syn::Path = syn::parse_quote!(::kirin::ir);
        let builder = GenericsBuilder::new(&ir_path);

        let base: syn::Generics = syn::parse_quote!(<T: CompileTimeValue>);
        let result = builder.with_language(&base);

        insta::assert_snapshot!("with_language_existing_type", format_generics(&result));
    }
}
