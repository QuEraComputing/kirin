//! Code generation for the `WithAbstractSyntaxTree` derive macro.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{FieldKind, collect_fields};
use crate::generics::GenericsBuilder;
use kirin_derive_core::codegen::{renamed_field_idents, tuple_field_idents};

/// Generator for the `WithAbstractSyntaxTree` trait implementation.
pub struct GenerateWithAbstractSyntaxTree {
    crate_path: syn::Path,
}

impl GenerateWithAbstractSyntaxTree {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        let crate_path = ir_input
            .extra_attrs
            .crate_path
            .clone()
            .or(ir_input.attrs.crate_path.clone())
            .unwrap_or_else(|| syn::parse_quote!(::kirin_chumsky));
        Self { crate_path }
    }

    /// Generates the AST type and `WithAbstractSyntaxTree` implementation.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_generics = self.build_ast_generics(ir_input);

        let ast_definition = self.generate_ast_definition(ir_input, &ast_name, &ast_generics);
        let trait_impls = self.generate_derive_impls(ir_input, &ast_name, &ast_generics);
        let trait_impl = self.generate_trait_impl(ir_input, &ast_name, &ast_generics);

        quote! {
            #ast_definition
            #trait_impls
            #trait_impl
        }
    }

    fn build_ast_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.crate_path).with_language(&ir_input.generics)
    }

    fn generate_ast_definition(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let (_, ty_generics, _) = ast_generics.split_for_impl();

        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(data) => {
                let fields = self.generate_struct_fields(&data.0, true);
                let is_tuple = data.0.is_tuple_style();

                if is_tuple {
                    quote! {
                        pub struct #ast_name #ty_generics
                        where
                            Language: #crate_path::LanguageParser<'tokens, 'src>,
                        (
                            #fields
                        );
                    }
                } else {
                    quote! {
                        pub struct #ast_name #ty_generics
                        where
                            Language: #crate_path::LanguageParser<'tokens, 'src>,
                        {
                            #fields
                        }
                    }
                }
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                let variants = self.generate_enum_variants(data);
                quote! {
                    pub enum #ast_name #ty_generics
                    where
                        Language: #crate_path::LanguageParser<'tokens, 'src>,
                    {
                        #variants
                    }
                }
            }
        }
    }

    fn generate_derive_impls(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();

        // Generate Debug impl
        let debug_impl = self.generate_debug_impl(ir_input, ast_name, ast_generics);

        // Generate Clone impl
        let clone_impl = self.generate_clone_impl(ir_input, ast_name, ast_generics);

        // Generate PartialEq impl
        let partialeq_impl = self.generate_partialeq_impl(ir_input, ast_name, ast_generics);

        quote! {
            impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
            where
                Language: #crate_path::LanguageParser<'tokens, 'src>,
            {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    #debug_impl
                }
            }

            impl #impl_generics ::core::clone::Clone for #ast_name #ty_generics
            where
                Language: #crate_path::LanguageParser<'tokens, 'src>,
            {
                fn clone(&self) -> Self {
                    #clone_impl
                }
            }

            impl #impl_generics ::core::cmp::PartialEq for #ast_name #ty_generics
            where
                Language: #crate_path::LanguageParser<'tokens, 'src>,
            {
                fn eq(&self, other: &Self) -> bool {
                    #partialeq_impl
                }
            }
        }
    }

    fn generate_debug_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        _ast_generics: &syn::Generics,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(data) => {
                let name = ast_name.to_string();
                let is_tuple = data.0.is_tuple_style();

                if is_tuple {
                    let field_names = tuple_field_idents("f", data.0.field_count());
                    let patterns = quote! { Self(#(#field_names),*) };
                    let debug_fields = field_names.iter().fold(
                        quote! { f.debug_tuple(#name) },
                        |acc, field| quote! { #acc.field(&#field) },
                    );
                    quote! {
                        let #patterns = self;
                        #debug_fields.finish()
                    }
                } else {
                    let fields = data.0.named_field_idents();
                    let patterns = quote! { Self { #(#fields),* } };
                    let debug_fields =
                        fields
                            .iter()
                            .fold(quote! { f.debug_struct(#name) }, |acc, field| {
                                let field_name = field.to_string();
                                quote! { #acc.field(#field_name, &#field) }
                            });
                    quote! {
                        let #patterns = self;
                        #debug_fields.finish()
                    }
                }
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let name_str = name.to_string();
                        let is_tuple = variant.is_tuple_style();

                        if is_tuple {
                            let field_count = variant.field_count();
                            if field_count == 0 {
                                quote! {
                                    Self::#name => f.write_str(#name_str)
                                }
                            } else {
                                let field_names = tuple_field_idents("f", field_count);
                                let debug_fields = field_names.iter().fold(
                                    quote! { f.debug_tuple(#name_str) },
                                    |acc, field| quote! { #acc.field(&#field) },
                                );
                                quote! {
                                    Self::#name(#(#field_names),*) => #debug_fields.finish()
                                }
                            }
                        } else {
                            let fields = variant.named_field_idents();
                            if fields.is_empty() {
                                quote! {
                                    Self::#name {} => f.write_str(#name_str)
                                }
                            } else {
                                let debug_fields = fields.iter().fold(
                                    quote! { f.debug_struct(#name_str) },
                                    |acc, field| {
                                        let field_name = field.to_string();
                                        quote! { #acc.field(#field_name, &#field) }
                                    },
                                );
                                quote! {
                                    Self::#name { #(#fields),* } => #debug_fields.finish()
                                }
                            }
                        }
                    })
                    .collect();

                quote! {
                    match self {
                        #(#arms),*
                    }
                }
            }
        }
    }

    fn generate_clone_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        _ast_name: &syn::Ident,
        _ast_generics: &syn::Generics,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(data) => {
                let is_tuple = data.0.is_tuple_style();

                if is_tuple {
                    let field_names = tuple_field_idents("f", data.0.field_count());
                    let patterns = quote! { Self(#(#field_names),*) };
                    let clones = quote! { Self(#(#field_names.clone()),*) };
                    quote! {
                        let #patterns = self;
                        #clones
                    }
                } else {
                    let fields = data.0.named_field_idents();
                    let patterns = quote! { Self { #(#fields),* } };
                    let clones: Vec<_> = fields.iter().map(|f| quote! { #f: #f.clone() }).collect();
                    quote! {
                        let #patterns = self;
                        Self { #(#clones),* }
                    }
                }
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let is_tuple = variant.is_tuple_style();

                        if is_tuple {
                            let field_count = variant.field_count();
                            if field_count == 0 {
                                quote! { Self::#name => Self::#name }
                            } else {
                                let field_names = tuple_field_idents("f", field_count);
                                let clones = quote! { Self::#name(#(#field_names.clone()),*) };
                                quote! {
                                    Self::#name(#(#field_names),*) => #clones
                                }
                            }
                        } else {
                            let fields = variant.named_field_idents();
                            if fields.is_empty() {
                                quote! { Self::#name {} => Self::#name {} }
                            } else {
                                let clones: Vec<_> =
                                    fields.iter().map(|f| quote! { #f: #f.clone() }).collect();
                                quote! {
                                    Self::#name { #(#fields),* } => Self::#name { #(#clones),* }
                                }
                            }
                        }
                    })
                    .collect();

                quote! {
                    match self {
                        #(#arms),*
                    }
                }
            }
        }
    }

    fn generate_partialeq_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        _ast_name: &syn::Ident,
        _ast_generics: &syn::Generics,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(data) => {
                let is_tuple = data.0.is_tuple_style();

                if is_tuple {
                    let field_count = data.0.field_count();
                    let self_fields = tuple_field_idents("s", field_count);
                    let other_fields = tuple_field_idents("o", field_count);
                    let comparisons = self_fields.iter().zip(&other_fields).map(|(s, o)| {
                        quote! { #s == #o }
                    });
                    quote! {
                        let Self(#(#self_fields),*) = self;
                        let Self(#(#other_fields),*) = other;
                        true #(&& #comparisons)*
                    }
                } else {
                    let fields = data.0.named_field_idents();
                    let self_fields = renamed_field_idents("s_", &fields);
                    let other_fields = renamed_field_idents("o_", &fields);
                    let self_pattern: Vec<_> = fields
                        .iter()
                        .zip(&self_fields)
                        .map(|(f, s)| quote! { #f: #s })
                        .collect();
                    let other_pattern: Vec<_> = fields
                        .iter()
                        .zip(&other_fields)
                        .map(|(f, o)| quote! { #f: #o })
                        .collect();
                    let comparisons = self_fields.iter().zip(&other_fields).map(|(s, o)| {
                        quote! { #s == #o }
                    });
                    quote! {
                        let Self { #(#self_pattern),* } = self;
                        let Self { #(#other_pattern),* } = other;
                        true #(&& #comparisons)*
                    }
                }
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let is_tuple = variant.is_tuple_style();

                        if is_tuple {
                            let field_count = variant.field_count();
                            if field_count == 0 {
                                quote! {
                                    (Self::#name, Self::#name) => true
                                }
                            } else {
                                let self_fields = tuple_field_idents("s", field_count);
                                let other_fields = tuple_field_idents("o", field_count);
                                let comparisons = self_fields.iter().zip(&other_fields).map(|(s, o)| {
                                    quote! { #s == #o }
                                });
                                quote! {
                                    (Self::#name(#(#self_fields),*), Self::#name(#(#other_fields),*)) => {
                                        true #(&& #comparisons)*
                                    }
                                }
                            }
                        } else {
                            let fields = variant.named_field_idents();
                            if fields.is_empty() {
                                quote! {
                                    (Self::#name {}, Self::#name {}) => true
                                }
                            } else {
                                let self_fields = renamed_field_idents("s_", &fields);
                                let other_fields = renamed_field_idents("o_", &fields);
                                let self_pattern: Vec<_> = fields.iter().zip(&self_fields)
                                    .map(|(f, s)| quote! { #f: #s })
                                    .collect();
                                let other_pattern: Vec<_> = fields.iter().zip(&other_fields)
                                    .map(|(f, o)| quote! { #f: #o })
                                    .collect();
                                let comparisons = self_fields.iter().zip(&other_fields).map(|(s, o)| {
                                    quote! { #s == #o }
                                });
                                quote! {
                                    (Self::#name { #(#self_pattern),* }, Self::#name { #(#other_pattern),* }) => {
                                        true #(&& #comparisons)*
                                    }
                                }
                            }
                        }
                    })
                    .collect();

                quote! {
                    match (self, other) {
                        #(#arms,)*
                        _ => false
                    }
                }
            }
        }
    }

    fn generate_struct_fields(
        &self,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        with_pub: bool,
    ) -> TokenStream {
        let collected = collect_fields(stmt);
        let mut fields = Vec::new();

        for field in &collected {
            let ty = self.field_ast_type(&field.collection, &field.kind);
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

        let is_tuple = stmt.is_tuple_style();

        if is_tuple {
            quote! { #(#fields),* }
        } else {
            quote! { #(#fields,)* }
        }
    }

    fn generate_enum_variants(
        &self,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
    ) -> TokenStream {
        let variants: Vec<TokenStream> = data
            .variants
            .iter()
            .map(|variant| {
                let name = &variant.name;

                // Check if this is a wrapper variant
                if let Some(wrapper) = &variant.wraps {
                    let wrapped_ty = &wrapper.ty;
                    let crate_path = &self.crate_path;
                    return quote! {
                        #name(<#wrapped_ty as #crate_path::WithAbstractSyntaxTree<'tokens, 'src, Language>>::AbstractSyntaxTreeNode)
                    };
                }

                // For enum variants, don't use `pub`
                let fields = self.generate_struct_fields(variant, false);
                let is_tuple = variant.is_tuple_style();

                if is_tuple {
                    quote! { #name(#fields) }
                } else {
                    quote! { #name { #fields } }
                }
            })
            .collect();

        quote! { #(#variants,)* }
    }

    fn field_ast_type(
        &self,
        collection: &kirin_derive_core::ir::fields::Collection,
        kind: &FieldKind,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let base = match kind {
            FieldKind::SSAValue => {
                quote! { #crate_path::SSAValue<'tokens, 'src, Language> }
            }
            FieldKind::ResultValue => {
                quote! { #crate_path::ResultValue<'tokens, 'src, Language> }
            }
            FieldKind::Block => {
                // Block parser returns Spanned<Block>, so we need Spanned wrapper
                quote! { #crate_path::Spanned<#crate_path::Block<'tokens, 'src, Language>> }
            }
            FieldKind::Successor => {
                quote! { #crate_path::BlockLabel<'src> }
            }
            FieldKind::Region => {
                quote! { #crate_path::Region<'tokens, 'src, Language> }
            }
            FieldKind::Value(ty) => {
                quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::Output }
            }
        };

        match collection {
            kirin_derive_core::ir::fields::Collection::Single => base,
            kirin_derive_core::ir::fields::Collection::Vec => quote! { Vec<#base> },
            kirin_derive_core::ir::fields::Collection::Option => quote! { Option<#base> },
        }
    }

    fn generate_trait_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let name = &ir_input.name;
        let (impl_generics, ty_generics, where_clause) = ast_generics.split_for_impl();

        // Get the original type's type generics (without 'tokens, 'src, Language)
        let (_, original_ty_generics, _) = ir_input.generics.split_for_impl();

        quote! {
            impl #impl_generics #crate_path::WithAbstractSyntaxTree<'tokens, 'src, Language> for #name #original_ty_generics
            #where_clause
            {
                type AbstractSyntaxTreeNode = #ast_name #ty_generics;
            }
        }
    }
}
