use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind;

use crate::codegen::{collect_wrapper_types, filter_ast_fields, get_fields_in_format};

use super::GenerateAST;

impl GenerateAST {
    pub(super) fn generate_ast_definition(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let ast_generics = crate::codegen::build_ast_generics(&ir_input.generics, false);
        let (impl_generics, _, _) = ast_generics.split_for_impl();

        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();
        let has_original_type_params = !type_params.is_empty();

        let phantom = if has_original_type_params {
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), #(#type_params,)* TypeOutput, LanguageOutput)> }
        } else {
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), TypeOutput, LanguageOutput)> }
        };

        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let crate_path = &self.config.crate_path;
        let has_parser_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens })
            .collect();

        let wrapper_types = collect_wrapper_types(ir_input);
        let has_dialect_parser_bounds: Vec<_> = wrapper_types
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();
        let has_wrapper_variants = !wrapper_types.is_empty();

        let base_bounds = quote! {
            TypeOutput: Clone + PartialEq + 'tokens,
            LanguageOutput: Clone + PartialEq + 'tokens,
        };

        let needs_manual_impls = has_original_type_params || has_wrapper_variants;

        match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Struct(data) => {
                let fields = self.generate_struct_fields(ir_input, &data.0, true, ast_name);
                let is_tuple = data.0.is_tuple_style();

                if needs_manual_impls {
                    let manual_impls = self.generate_manual_struct_trait_impls(
                        ir_input,
                        &data.0,
                        ast_name,
                        &ast_generics,
                        &base_bounds,
                        &has_parser_bounds,
                        is_tuple,
                    );

                    if is_tuple {
                        quote! {
                            #[doc(hidden)]
                            pub struct #ast_name #impl_generics (
                                #fields,
                                #phantom,
                            )
                            where
                                #base_bounds
                                #(#has_parser_bounds,)*;

                            #manual_impls
                        }
                    } else {
                        quote! {
                            #[doc(hidden)]
                            pub struct #ast_name #impl_generics
                            where
                                #base_bounds
                                #(#has_parser_bounds,)*
                            {
                                #fields
                                #[doc(hidden)]
                                _marker: #phantom,
                            }

                            #manual_impls
                        }
                    }
                } else if is_tuple {
                    quote! {
                        #[derive(Clone, Debug, PartialEq)]
                        #[doc(hidden)]
                        pub struct #ast_name #impl_generics (
                            #fields,
                            #phantom,
                        )
                        where
                            #base_bounds
                            #(#has_parser_bounds,)*;
                    }
                } else {
                    quote! {
                        #[derive(Clone, Debug, PartialEq)]
                        #[doc(hidden)]
                        pub struct #ast_name #impl_generics
                        where
                            #base_bounds
                            #(#has_parser_bounds,)*
                        {
                            #fields
                            #[doc(hidden)]
                            _marker: #phantom,
                        }
                    }
                }
            }
            kirin_derive_toolkit::ir::Data::Enum(data) => {
                let variants = self.generate_enum_variants(ir_input, data, ast_name);

                if needs_manual_impls {
                    let manual_impls = self.generate_manual_trait_impls_for_wrapper_enum(
                        ir_input,
                        data,
                        ast_name,
                        &ast_generics,
                        &base_bounds,
                        &has_parser_bounds,
                        &has_dialect_parser_bounds,
                    );

                    quote! {
                        #[doc(hidden)]
                        pub enum #ast_name #impl_generics
                        where
                            #base_bounds
                            #(#has_parser_bounds,)*
                            #(#has_dialect_parser_bounds,)*
                        {
                            #variants
                            #[doc(hidden)]
                            __Marker(#phantom, ::core::convert::Infallible),
                        }

                        #manual_impls
                    }
                } else {
                    quote! {
                        #[derive(Clone, Debug, PartialEq)]
                        #[doc(hidden)]
                        pub enum #ast_name #impl_generics
                        where
                            #base_bounds
                            #(#has_parser_bounds,)*
                        {
                            #variants
                            #[doc(hidden)]
                            __Marker(#phantom, ::core::convert::Infallible),
                        }
                    }
                }
            }
        }
    }

    pub(super) fn generate_struct_fields(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
        with_pub: bool,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let collected = stmt.collect_fields();
        let fields_in_fmt = get_fields_in_format(ir_input, stmt);
        let is_tuple = stmt.is_tuple_style();

        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        let mut filtered: Vec<_> = filter_ast_fields(&collected, &fields_in_fmt);

        if is_tuple {
            filtered.sort_by_key(|f| f.index);
        }

        let mut fields = Vec::new();

        for field in &filtered {
            let ty = self.field_ast_type(field, &field.collection, ast_name, &type_params);
            if let Some(ident) = &field.ident {
                if with_pub {
                    fields.push(quote! { pub #ident: #ty });
                } else {
                    fields.push(quote! { #ident: #ty });
                }
            } else if with_pub {
                fields.push(quote! { pub #ty });
            } else {
                fields.push(quote! { #ty });
            }
        }

        if is_tuple {
            quote! { #(#fields),* }
        } else {
            quote! { #(#fields,)* }
        }
    }

    pub(super) fn generate_enum_variants(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        data: &kirin_derive_toolkit::ir::DataEnum<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        use kirin_derive_toolkit::ir::VariantRef;
        let crate_path = &self.config.crate_path;

        let variants: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, wrapper, .. } => {
                    let wrapped_ty = &wrapper.ty;
                    quote! {
                        #name(<#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, LanguageOutput>)
                    }
                }
                VariantRef::Regular { name, stmt } => {
                    let fields = self.generate_struct_fields(ir_input, stmt, false, ast_name);
                    let is_tuple = stmt.is_tuple_style();

                    if is_tuple {
                        quote! { #name(#fields) }
                    } else {
                        quote! { #name { #fields } }
                    }
                }
            })
            .collect();

        quote! { #(#variants,)* }
    }

    pub(super) fn field_ast_type(
        &self,
        field: &kirin_derive_toolkit::ir::fields::FieldInfo<ChumskyLayout>,
        collection: &kirin_derive_toolkit::ir::fields::Collection,
        ast_name: &syn::Ident,
        type_params: &[TokenStream],
    ) -> TokenStream {
        let base = field_kind::ast_type(
            field,
            &self.config.crate_path,
            ast_name,
            &self.config.ir_type,
            type_params,
        );
        collection.wrap_type(base)
    }
}
