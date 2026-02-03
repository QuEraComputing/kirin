//! Code generation for AST types corresponding to dialect definitions.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{FieldKind, collect_fields};

use kirin_derive_core::codegen::deduplicate_types;

use super::{GeneratorConfig, collect_all_value_types_needing_bounds, filter_ast_fields, get_fields_in_format};

/// Generator for AST type definitions.
///
/// This generates the AST type (e.g., `MyDialectAST`) that corresponds to a dialect
/// definition. The AST type is used during parsing to represent the syntax tree
/// before it's converted to IR.
pub struct GenerateAST {
    config: GeneratorConfig,
}

impl GenerateAST {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        Self {
            config: GeneratorConfig::new(ir_input),
        }
    }

    /// Generates the AST type definition with derive(Clone, Debug, PartialEq).
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_generics = self.config.build_ast_generics(ir_input);

        self.generate_ast_definition(ir_input, &ast_name, &ast_generics)
    }

    /// Collects all types that contain type parameters and need HasParser bounds.
    ///
    /// This includes:
    /// - Value field types that contain type parameters
    /// - type_lattice if it contains type parameters
    fn collect_value_types_needing_bounds(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> Vec<syn::Type> {
        let mut all_types = Vec::new();

        // Collect type parameter names
        let type_param_names: Vec<String> = ir_input
            .generics
            .type_params()
            .map(|p| p.ident.to_string())
            .collect();

        // Check if type_lattice contains any type parameter
        let type_lattice = &self.config.type_lattice;
        let type_lattice_ty: syn::Type = syn::parse_quote!(#type_lattice);
        for param_name in &type_param_names {
            if kirin_derive_core::misc::is_type(&type_lattice_ty, param_name.as_str())
                || kirin_derive_core::misc::is_type_in_generic(&type_lattice_ty, param_name.as_str())
            {
                all_types.push(type_lattice_ty.clone());
                break;
            }
        }

        // Collect value field types from all statements
        all_types.extend(collect_all_value_types_needing_bounds(ir_input));
        deduplicate_types(&mut all_types);

        all_types
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

        // Collect value types that need HasParser bounds
        // These types need HasParser<'tokens, 'src> + 'tokens bound
        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let crate_path = &self.config.crate_path;
        let has_parser_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens })
            .collect();

        // AST types need `Language: Dialect` bound plus HasParser bounds for value types.
        // Field types use the concrete type_lattice directly (not the associated type).
        // Block/Region fields use the concrete AST type name to avoid circular trait bounds.
        // We use #[derive(Clone, Debug, PartialEq)] - the PhantomData<fn() -> T> trick
        // ensures these traits work without requiring bounds on T.
        let ir_path = &self.config.ir_path;
        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(data) => {
                let fields = self.generate_struct_fields(ir_input, &data.0, true, ast_name);
                let is_tuple = data.0.is_tuple_style();

                if is_tuple {
                    // For tuple structs, the where clause must come after the tuple body
                    quote! {
                        #[derive(Clone, Debug, PartialEq)]
                        pub struct #ast_name #ty_generics (
                            #fields,
                            #phantom,
                        )
                        where
                            Language: #ir_path::Dialect,
                            #(#has_parser_bounds,)*;
                    }
                } else {
                    quote! {
                        #[derive(Clone, Debug, PartialEq)]
                        pub struct #ast_name #ty_generics
                        where
                            Language: #ir_path::Dialect,
                            #(#has_parser_bounds,)*
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
                    #[derive(Clone, Debug, PartialEq)]
                    pub enum #ast_name #ty_generics
                    where
                        Language: #ir_path::Dialect,
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

    fn generate_struct_fields(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        with_pub: bool,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let collected = collect_fields(stmt);
        let fields_in_fmt = get_fields_in_format(ir_input, stmt);
        let is_tuple = stmt.is_tuple_style();

        // Filter to only fields needed in AST
        let mut filtered: Vec<_> = filter_ast_fields(&collected, &fields_in_fmt);

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
        use kirin_derive_core::ir::VariantRef;

        let variants: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, wrapper, .. } => {
                    let wrapped_ty = &wrapper.ty;
                    let crate_path = &self.config.crate_path;
                    // Use HasParser::Output to get the AST type for wrapped dialects
                    quote! {
                        #name(<#wrapped_ty as #crate_path::HasParser<'tokens, 'src>>::Output)
                    }
                }
                VariantRef::Regular { name, stmt } => {
                    // For enum variants, don't use `pub`
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

    fn field_ast_type(
        &self,
        collection: &kirin_derive_core::ir::fields::Collection,
        kind: &FieldKind,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let base = kind.ast_type(&self.config.crate_path, ast_name, &self.config.type_lattice);
        collection.wrap_type(base)
    }
}
