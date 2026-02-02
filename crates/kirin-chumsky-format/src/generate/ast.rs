//! Code generation for AST types corresponding to dialect definitions.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{CollectedField, FieldKind, collect_fields, fields_in_format};
use crate::format::Format;
use crate::generics::GenericsBuilder;

/// Generator for AST type definitions.
///
/// This generates the AST type (e.g., `MyDialectAST`) that corresponds to a dialect
/// definition. The AST type is used during parsing to represent the syntax tree
/// before it's converted to IR.
pub struct GenerateAST {
    crate_path: syn::Path,
    ir_path: syn::Path,
    type_lattice: syn::Path,
}

impl GenerateAST {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        let crate_path = ir_input
            .extra_attrs
            .crate_path
            .clone()
            .or(ir_input.attrs.crate_path.clone())
            .unwrap_or_else(|| syn::parse_quote!(::kirin_chumsky));
        // IR path comes from #[kirin(crate = ...)] which defaults to ::kirin_ir
        let ir_path = ir_input
            .attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| syn::parse_quote!(::kirin_ir));
        let type_lattice = ir_input.attrs.type_lattice.clone();
        Self {
            crate_path,
            ir_path,
            type_lattice,
        }
    }

    /// Generates the AST type definition with Debug, Clone, and PartialEq impls.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_generics = self.build_ast_generics(ir_input);

        let ast_definition = self.generate_ast_definition(ir_input, &ast_name, &ast_generics);
        let trait_impls = self.generate_derive_impls(ir_input, &ast_name, &ast_generics);

        quote! {
            #ast_definition
            #trait_impls
        }
    }

    /// Generates only the AST type definition without trait impls.
    /// Useful for testing to get cleaner output.
    pub fn generate_definition_only(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_generics = self.build_ast_generics(ir_input);
        self.generate_ast_definition(ir_input, &ast_name, &ast_generics)
    }

    fn build_ast_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.ir_path).with_language(&ir_input.generics)
    }

    fn generate_ast_definition(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
    ) -> TokenStream {
        let (_, ty_generics, _) = ast_generics.split_for_impl();

        // We use PhantomData to make all generic parameters used ('tokens, 'src, Language).
        // Using fn() -> ... makes them covariant and doesn't require Clone/Debug/etc.
        let phantom =
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), Language)> };

        // AST types only need `Language: Dialect` bound.
        // Field types use the concrete type_lattice directly (not the associated type).
        // Block/Region fields use the concrete AST type name to avoid circular trait bounds.
        let ir_path = &self.ir_path;
        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(data) => {
                let fields = self.generate_struct_fields(ir_input, &data.0, true, ast_name);
                let is_tuple = data.0.is_tuple_style();

                if is_tuple {
                    quote! {
                        pub struct #ast_name #ty_generics
                        where
                            Language: #ir_path::Dialect,
                        (
                            #fields,
                            #phantom,
                        );
                    }
                } else {
                    quote! {
                        pub struct #ast_name #ty_generics
                        where
                            Language: #ir_path::Dialect,
                        {
                            #fields
                            #[doc(hidden)]
                            _marker: #phantom,
                        }
                    }
                }
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                let variants = self.generate_enum_variants(ir_input, data, ast_name);
                quote! {
                    pub enum #ast_name #ty_generics
                    where
                        Language: #ir_path::Dialect,
                    {
                        #variants
                        #[doc(hidden)]
                        __Marker(#phantom, ::core::convert::Infallible),
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
        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();

        // Generate Debug impl
        let debug_impl = self.generate_debug_impl(ir_input, ast_name, ast_generics);

        // Generate Clone impl
        let clone_impl = self.generate_clone_impl(ir_input, ast_name, ast_generics);

        // Generate PartialEq impl
        let partialeq_impl = self.generate_partialeq_impl(ir_input, ast_name, ast_generics);

        let ir_path = &self.ir_path;
        quote! {
            impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
            where
                Language: #ir_path::Dialect,
            {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    #debug_impl
                }
            }

            impl #impl_generics ::core::clone::Clone for #ast_name #ty_generics
            where
                Language: #ir_path::Dialect,
            {
                fn clone(&self) -> Self {
                    #clone_impl
                }
            }

            impl #impl_generics ::core::cmp::PartialEq for #ast_name #ty_generics
            where
                Language: #ir_path::Dialect,
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
                let bindings = data.0.field_bindings("f");
                let fields = &bindings.field_idents;

                let (pattern, debug_fields) = if bindings.is_tuple {
                    // Include _marker PhantomData at the end, ignored with _
                    let pattern = quote! { Self(#(#fields,)* _) };
                    let debug = fields.iter().fold(
                        quote! { f.debug_tuple(#name) },
                        |acc, field| quote! { #acc.field(&#field) },
                    );
                    (pattern, debug)
                } else {
                    let orig_fields = &bindings.original_field_names;
                    let pat: Vec<_> = orig_fields
                        .iter()
                        .zip(fields)
                        .map(|(f, b)| quote! { #f: #b })
                        .collect();
                    // Include _marker field, ignored with ..
                    let pattern = quote! { Self { #(#pat,)* .. } };
                    let debug = orig_fields.iter().zip(fields).fold(
                        quote! { f.debug_struct(#name) },
                        |acc, (orig, field)| {
                            let field_name = orig.to_string();
                            quote! { #acc.field(#field_name, &#field) }
                        },
                    );
                    (pattern, debug)
                };

                quote! {
                    let #pattern = self;
                        #debug_fields.finish()
                }
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let name_str = name.to_string();
                        let bindings = variant.field_bindings("f");
                        let fields = &bindings.field_idents;

                        if bindings.is_empty() {
                            if bindings.is_tuple {
                                quote! { Self::#name => f.write_str(#name_str) }
                            } else {
                                quote! { Self::#name {} => f.write_str(#name_str) }
                            }
                        } else if bindings.is_tuple {
                            let debug_fields = fields.iter().fold(
                                quote! { f.debug_tuple(#name_str) },
                                |acc, field| quote! { #acc.field(&#field) },
                            );
                            quote! { Self::#name(#(#fields),*) => #debug_fields.finish() }
                        } else {
                            let orig_fields = &bindings.original_field_names;
                            let pat: Vec<_> = orig_fields
                                .iter()
                                .zip(fields)
                                .map(|(f, b)| quote! { #f: #b })
                                .collect();
                            let debug_fields = orig_fields.iter().zip(fields).fold(
                                quote! { f.debug_struct(#name_str) },
                                |acc, (orig, field)| {
                                    let field_name = orig.to_string();
                                    quote! { #acc.field(#field_name, &#field) }
                                },
                            );
                            quote! { Self::#name { #(#pat),* } => #debug_fields.finish() }
                        }
                    })
                    .collect();

                // The __Marker variant is uninhabited (contains Infallible), so this is unreachable
                quote! {
                    match self {
                        #(#arms,)*
                        Self::__Marker(_, unreachable) => match *unreachable {},
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
                let bindings = data.0.field_bindings("f");
                let fields = &bindings.field_idents;

                let (pattern, cloned) = if bindings.is_tuple {
                    // Include _marker PhantomData at the end
                    (
                        quote! { Self(#(#fields,)* _) },
                        quote! { Self(#(#fields.clone(),)* ::core::marker::PhantomData) },
                    )
                } else {
                    let orig_fields = &bindings.original_field_names;
                    let pat: Vec<_> = orig_fields
                        .iter()
                        .zip(fields)
                        .map(|(f, b)| quote! { #f: #b })
                        .collect();
                    let clones: Vec<_> = orig_fields
                        .iter()
                        .zip(fields)
                        .map(|(f, b)| quote! { #f: #b.clone() })
                        .collect();
                    // Include _marker field
                    (
                        quote! { Self { #(#pat,)* .. } },
                        quote! { Self { #(#clones,)* _marker: ::core::marker::PhantomData } },
                    )
                };

                quote! {
                    let #pattern = self;
                    #cloned
                }
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let bindings = variant.field_bindings("f");
                        let fields = &bindings.field_idents;

                        if bindings.is_empty() {
                            if bindings.is_tuple {
                                quote! { Self::#name => Self::#name }
                            } else {
                                quote! { Self::#name {} => Self::#name {} }
                            }
                        } else if bindings.is_tuple {
                            quote! {
                                Self::#name(#(#fields),*) => Self::#name(#(#fields.clone()),*)
                            }
                        } else {
                            let orig_fields = &bindings.original_field_names;
                            let pat: Vec<_> = orig_fields
                                .iter()
                                .zip(fields)
                                .map(|(f, b)| quote! { #f: #b })
                                .collect();
                            let clones: Vec<_> = orig_fields
                                .iter()
                                .zip(fields)
                                .map(|(f, b)| quote! { #f: #b.clone() })
                                .collect();
                            quote! {
                                Self::#name { #(#pat),* } => Self::#name { #(#clones),* }
                            }
                        }
                    })
                    .collect();

                // The __Marker variant is uninhabited (contains Infallible), so this is unreachable
                quote! {
                    match self {
                        #(#arms,)*
                        Self::__Marker(_, unreachable) => match *unreachable {},
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
                let self_bindings = data.0.field_bindings("s");
                let other_bindings = self_bindings.with_prefix("o");
                let self_fields = &self_bindings.field_idents;
                let other_fields = &other_bindings.field_idents;

                let comparisons = self_fields.iter().zip(other_fields).map(|(s, o)| {
                    quote! { #s == #o }
                });

                let (self_pattern, other_pattern) = if self_bindings.is_tuple {
                    // Include _marker PhantomData at the end, ignored with _
                    (
                        quote! { Self(#(#self_fields,)* _) },
                        quote! { Self(#(#other_fields,)* _) },
                    )
                } else {
                    let orig_fields = &self_bindings.original_field_names;
                    let self_pat: Vec<_> = orig_fields
                        .iter()
                        .zip(self_fields)
                        .map(|(f, s)| quote! { #f: #s })
                        .collect();
                    let other_pat: Vec<_> = orig_fields
                        .iter()
                        .zip(other_fields)
                        .map(|(f, o)| quote! { #f: #o })
                        .collect();
                    // Include _marker field, ignored with ..
                    (
                        quote! { Self { #(#self_pat,)* .. } },
                        quote! { Self { #(#other_pat,)* .. } },
                    )
                };

                quote! {
                    let #self_pattern = self;
                    let #other_pattern = other;
                        true #(&& #comparisons)*
                }
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let self_bindings = variant.field_bindings("s");
                        let other_bindings = self_bindings.with_prefix("o");
                        let self_fields = &self_bindings.field_idents;
                        let other_fields = &other_bindings.field_idents;

                        if self_bindings.is_empty() {
                            if self_bindings.is_tuple {
                                quote! { (Self::#name, Self::#name) => true }
                            } else {
                                quote! { (Self::#name {}, Self::#name {}) => true }
                            }
                        } else if self_bindings.is_tuple {
                            let comparisons = self_fields.iter().zip(other_fields).map(|(s, o)| {
                                    quote! { #s == #o }
                                });
                                quote! {
                                    (Self::#name(#(#self_fields),*), Self::#name(#(#other_fields),*)) => {
                                        true #(&& #comparisons)*
                                }
                            }
                        } else {
                            let orig_fields = &self_bindings.original_field_names;
                            let self_pat: Vec<_> = orig_fields
                                    .iter()
                                .zip(self_fields)
                                    .map(|(f, s)| quote! { #f: #s })
                                    .collect();
                            let other_pat: Vec<_> = orig_fields
                                .iter()
                                .zip(other_fields)
                                    .map(|(f, o)| quote! { #f: #o })
                                    .collect();
                            let comparisons = self_fields.iter().zip(other_fields).map(|(s, o)| {
                                    quote! { #s == #o }
                                });
                                quote! {
                                (Self::#name { #(#self_pat),* }, Self::#name { #(#other_pat),* }) => {
                                        true #(&& #comparisons)*
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

    /// Gets the format string for a statement, checking extra_attrs first.
    fn format_for_statement(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
    ) -> Option<String> {
        stmt.extra_attrs
            .format
            .clone()
            .or(stmt.attrs.format.clone())
            .or(ir_input.extra_attrs.format.clone())
    }

    /// Gets the set of field indices that are in the format string.
    fn get_fields_in_format(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
    ) -> HashSet<usize> {
        // If there's no format string, include all fields (wrapper variants, etc.)
        let Some(format_str) = self.format_for_statement(ir_input, stmt) else {
            return collect_fields(stmt).iter().map(|f| f.index).collect();
        };

        // Parse format string and get field indices
        match Format::parse(&format_str, None) {
            Ok(format) => fields_in_format(&format, stmt),
            Err(_) => {
                // If format parsing fails, include all fields (error will be reported elsewhere)
                collect_fields(stmt).iter().map(|f| f.index).collect()
            }
        }
    }

    /// Filters collected fields to only include those needed in the AST.
    ///
    /// Fields are included if they:
    /// - Are in the format string (will be parsed), OR
    /// - Don't have a default value (required)
    fn filter_fields_for_ast<'a>(
        &self,
        collected: &'a [CollectedField],
        fields_in_format: &HashSet<usize>,
    ) -> Vec<&'a CollectedField> {
        collected
            .iter()
            .filter(|f| fields_in_format.contains(&f.index) || f.default.is_none())
            .collect()
    }

    fn generate_struct_fields(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        with_pub: bool,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let collected = collect_fields(stmt);
        let fields_in_fmt = self.get_fields_in_format(ir_input, stmt);
        let is_tuple = stmt.is_tuple_style();

        // Filter to only fields needed in AST
        let mut filtered: Vec<_> = self.filter_fields_for_ast(&collected, &fields_in_fmt);

        // For tuple fields, sort by original index to match the IR field order.
        // For named fields, keep the category order (which matches iter_all_fields).
        if is_tuple {
            filtered.sort_by_key(|f| f.index);
        }

        let mut fields = Vec::new();

        for field in &filtered {
            let ty = self.field_ast_type(&field.collection, &field.kind, ast_name);
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

    fn generate_enum_variants(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
        ast_name: &syn::Ident,
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
                    // Use HasParser::Output to get the AST type for wrapped dialects
                    return quote! {
                        #name(<#wrapped_ty as #crate_path::HasParser<'tokens, 'src>>::Output)
                    };
                }

                // For enum variants, don't use `pub`
                let fields = self.generate_struct_fields(ir_input, variant, false, ast_name);
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
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let base = kind.ast_type(&self.crate_path, ast_name, &self.type_lattice);
        collection.wrap_type(base)
    }
}
