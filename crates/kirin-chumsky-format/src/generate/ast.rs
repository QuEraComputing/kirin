//! Code generation for AST types corresponding to dialect definitions.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{FieldKind, collect_fields};

use kirin_derive_core::codegen::deduplicate_types;

use super::{
    GeneratorConfig, collect_all_value_types_needing_bounds, filter_ast_fields,
    get_fields_in_format,
};

/// Generator for AST type definitions.
///
/// This generates the AST type (e.g., `MyDialectAST`) that corresponds to a dialect
/// definition. The AST type is used during parsing to represent the syntax tree
/// before it's converted to IR.
///
/// AST types are parameterized by `TypeOutput` and `LanguageOutput` instead of `Language: Dialect`.
/// This avoids GAT projection issues that cause infinite compilation times.
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
    ///
    /// For wrapper structs, no AST type is generated - the HasParser/HasDialectParser
    /// impls forward directly to the wrapped type's impls.
    ///
    /// This also generates the `ASTSelf` wrapper type for standalone use.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        // For wrapper structs, don't generate any AST type.
        // The HasParser/HasDialectParser impls will forward to the wrapped type.
        if let kirin_derive_core::ir::Data::Struct(data) = &ir_input.data {
            if data.0.wraps.is_some() {
                return TokenStream::new();
            }
        }

        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());

        let ast_def = self.generate_ast_definition(ir_input, &ast_name);
        let ast_self = self.generate_ast_self_wrapper(ir_input, &ast_name);

        quote! {
            #ast_def
            #ast_self
        }
    }

    /// Collects all types that contain type parameters and need HasParser bounds.
    ///
    /// This includes:
    /// - Value field types that contain type parameters
    /// - ir_type if it contains type parameters
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

        // Check if ir_type contains any type parameter
        let ir_type = &self.config.ir_type;
        let ir_type_ty: syn::Type = syn::parse_quote!(#ir_type);
        for param_name in &type_param_names {
            if kirin_derive_core::misc::is_type(&ir_type_ty, param_name.as_str())
                || kirin_derive_core::misc::is_type_in_generic(&ir_type_ty, param_name.as_str())
            {
                all_types.push(ir_type_ty.clone());
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
    ) -> TokenStream {
        let ast_generics = super::build_ast_generics(&ir_input.generics, false);
        // Use impl_generics to preserve original type parameter bounds (e.g., T: TypeLattice)
        let (impl_generics, _, _) = ast_generics.split_for_impl();

        // Extract original type parameters
        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();
        let has_original_type_params = !type_params.is_empty();

        // We use PhantomData to make all generic parameters used ('tokens, 'src, [original params], TypeOutput, LanguageOutput).
        // Using fn() -> ... makes them covariant and doesn't require Clone/Debug/etc.
        let phantom = if has_original_type_params {
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), #(#type_params,)* TypeOutput, LanguageOutput)> }
        } else {
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), TypeOutput, LanguageOutput)> }
        };

        // Collect value types that need HasParser bounds
        // These types need HasParser<'tokens, 'src> + 'tokens bound
        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let crate_path = &self.config.crate_path;
        let has_parser_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens })
            .collect();

        // Collect wrapper types that need HasDialectParser bounds
        // Wrapper enum variants use associated types, so the wrapped type needs this bound
        // Note: For wrapper enums, we generate manual trait impls with proper bounds,
        // so we only need the base HasDialectParser bound here.
        let wrapper_types = super::collect_wrapper_types(ir_input);
        let has_dialect_parser_bounds: Vec<_> = wrapper_types
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();
        let has_wrapper_variants = !wrapper_types.is_empty();

        // AST types need Clone + PartialEq bounds on TypeOutput and LanguageOutput.
        // No Dialect bounds needed anymore.
        let base_bounds = quote! {
            TypeOutput: Clone + PartialEq + 'tokens,
            LanguageOutput: Clone + PartialEq + 'tokens,
        };

        // Determine if we need manual trait impls (when we have original type params OR wrapper variants)
        // Standard #[derive] adds bounds on ALL type params, but we only want bounds on specific types.
        let needs_manual_impls = has_original_type_params || has_wrapper_variants;

        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(data) => {
                let fields = self.generate_struct_fields(ir_input, &data.0, true, ast_name);
                let is_tuple = data.0.is_tuple_style();

                if needs_manual_impls {
                    // Generate manual trait implementations to avoid incorrect bounds
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
                    // For tuple structs, the where clause must come after the tuple body
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
            kirin_derive_core::ir::Data::Enum(data) => {
                let variants = self.generate_enum_variants(ir_input, data, ast_name);

                if needs_manual_impls {
                    // For enums with wrapper variants or original type params,
                    // we can't use #[derive] because the standard derive macros
                    // don't handle GAT projections or phantom type params well.
                    // Instead, we manually implement Clone, Debug, PartialEq with proper bounds.
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

    /// Generates manual Clone, Debug, PartialEq implementations for structs.
    ///
    /// This is needed when the struct has original type parameters (like `T: TypeLattice`)
    /// because standard #[derive] adds bounds on ALL type params, but we only want bounds
    /// on specific types (TypeOutput, LanguageOutput, value types).
    fn generate_manual_struct_trait_impls(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        base_bounds: &TokenStream,
        has_parser_bounds: &[TokenStream],
        is_tuple: bool,
    ) -> TokenStream {
        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();
        let crate_path = &self.config.crate_path;
        let ir_type = &self.config.ir_type;

        // Collect value types that need Debug bounds (types containing type parameters)
        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let value_debug_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::Output: ::core::fmt::Debug })
            .collect();

        // Base where clause for Clone and PartialEq (no Debug bounds)
        let where_clause = quote! {
            where
                #base_bounds
                #(#has_parser_bounds,)*
        };

        // Debug where clause adds Debug bounds on the actual field types:
        // - AST fields use <ir_type as HasParser>::Output for type annotations
        // - Value fields use <ValueType as HasParser>::Output
        // - Block/Region fields use LanguageOutput for statements
        // So we need Debug on all these types.
        let debug_where_clause = quote! {
            where
                #base_bounds
                #(#has_parser_bounds,)*
                <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output: ::core::fmt::Debug,
                LanguageOutput: ::core::fmt::Debug,
                #(#value_debug_bounds,)*
        };

        // Get field info for pattern matching
        let collected = collect_fields(stmt);
        let fields_in_fmt = get_fields_in_format(ir_input, stmt);
        let filtered: Vec<_> = filter_ast_fields(&collected, &fields_in_fmt);

        if is_tuple {
            // Tuple struct
            let field_count = filtered.len();
            let field_indices: Vec<_> = (0..field_count)
                .map(|i| syn::Ident::new(&format!("f{}", i), proc_macro2::Span::call_site()))
                .collect();
            let clone_fields: Vec<_> = field_indices
                .iter()
                .map(|f| quote! { #f.clone() })
                .collect();
            let debug_fields: Vec<_> = field_indices
                .iter()
                .map(|f| quote! { .field(#f) })
                .collect();
            let eq_a: Vec<_> = (0..field_count)
                .map(|i| syn::Ident::new(&format!("a{}", i), proc_macro2::Span::call_site()))
                .collect();
            let eq_b: Vec<_> = (0..field_count)
                .map(|i| syn::Ident::new(&format!("b{}", i), proc_macro2::Span::call_site()))
                .collect();
            let eq_comparisons: Vec<_> = eq_a
                .iter()
                .zip(eq_b.iter())
                .map(|(a, b)| quote! { #a == #b })
                .collect();
            let eq_comparison = if eq_comparisons.is_empty() {
                quote! { true }
            } else {
                quote! { #(#eq_comparisons)&&* }
            };

            let ast_name_str = ast_name.to_string();

            quote! {
                impl #impl_generics Clone for #ast_name #ty_generics
                #where_clause
                {
                    fn clone(&self) -> Self {
                        let Self(#(#field_indices,)* _marker) = self;
                        Self(#(#clone_fields,)* ::core::marker::PhantomData)
                    }
                }

                impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
                #debug_where_clause
                {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let Self(#(#field_indices,)* _) = self;
                        f.debug_tuple(#ast_name_str)#(#debug_fields)*.finish()
                    }
                }

                impl #impl_generics PartialEq for #ast_name #ty_generics
                #where_clause
                {
                    fn eq(&self, other: &Self) -> bool {
                        let Self(#(#eq_a,)* _) = self;
                        let Self(#(#eq_b,)* _) = other;
                        #eq_comparison
                    }
                }
            }
        } else {
            // Named struct
            let field_names: Vec<_> = filtered.iter().filter_map(|f| f.ident.as_ref()).collect();
            let clone_fields: Vec<_> = field_names
                .iter()
                .map(|f| quote! { #f: self.#f.clone() })
                .collect();
            let debug_fields: Vec<_> = field_names
                .iter()
                .map(|f| {
                    let name_str = f.to_string();
                    quote! { .field(#name_str, &self.#f) }
                })
                .collect();
            let eq_comparisons: Vec<_> = field_names
                .iter()
                .map(|f| quote! { self.#f == other.#f })
                .collect();
            let eq_comparison = if eq_comparisons.is_empty() {
                quote! { true }
            } else {
                quote! { #(#eq_comparisons)&&* }
            };

            let ast_name_str = ast_name.to_string();

            quote! {
                impl #impl_generics Clone for #ast_name #ty_generics
                #where_clause
                {
                    fn clone(&self) -> Self {
                        Self {
                            #(#clone_fields,)*
                            _marker: ::core::marker::PhantomData,
                        }
                    }
                }

                impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
                #debug_where_clause
                {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        f.debug_struct(#ast_name_str)#(#debug_fields)*.finish()
                    }
                }

                impl #impl_generics PartialEq for #ast_name #ty_generics
                #where_clause
                {
                    fn eq(&self, other: &Self) -> bool {
                        #eq_comparison
                    }
                }
            }
        }
    }

    /// Generates the ASTSelf wrapper type for standalone use.
    ///
    /// This wrapper sets LanguageOutput = Self, creating a self-referential type
    /// that can be used with HasParser.
    fn generate_ast_self_wrapper(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let ast_self_name = syn::Ident::new(&format!("{}Self", ast_name), ir_input.name.span());
        let crate_path = &self.config.crate_path;

        // Extract original type parameters
        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        // Build the inner AST type reference
        let inner_ast_type = if type_params.is_empty() {
            quote! { #ast_name<'tokens, 'src, TypeOutput, #ast_self_name<'tokens, 'src, TypeOutput>> }
        } else {
            quote! { #ast_name<'tokens, 'src, #(#type_params,)* TypeOutput, #ast_self_name<'tokens, 'src, #(#type_params,)* TypeOutput>> }
        };

        // Build generics for ASTSelf definition: <'tokens, 'src, [original type params with bounds], TypeOutput>
        // Note: For the struct definition, use 'src: 'tokens bound syntax
        let ast_self_def_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src: 'tokens, TypeOutput> }
        } else {
            // Get original type parameters with their bounds
            let type_param_bounds: Vec<_> = ir_input
                .generics
                .type_params()
                .map(|p| {
                    let ident = &p.ident;
                    let bounds = &p.bounds;
                    if bounds.is_empty() {
                        quote! { #ident }
                    } else {
                        quote! { #ident: #bounds }
                    }
                })
                .collect();
            quote! { <'tokens, 'src: 'tokens, #(#type_param_bounds,)* TypeOutput> }
        };

        // Build generics for impl block: no bounds on lifetimes here, just list them
        let ast_self_impl_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput> }
        } else {
            // Get original type parameters with their bounds for impl
            let type_param_bounds: Vec<_> = ir_input
                .generics
                .type_params()
                .map(|p| {
                    let ident = &p.ident;
                    let bounds = &p.bounds;
                    if bounds.is_empty() {
                        quote! { #ident }
                    } else {
                        quote! { #ident: #bounds }
                    }
                })
                .collect();
            quote! { <'tokens, 'src, #(#type_param_bounds,)* TypeOutput> }
        };

        // Build type reference for impl Self type: <'tokens, 'src, [params], TypeOutput>
        let ast_self_ty_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput> }
        } else {
            quote! { <'tokens, 'src, #(#type_params,)* TypeOutput> }
        };

        // PhantomData for unused params
        let phantom = if type_params.is_empty() {
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), TypeOutput)> }
        } else {
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), #(#type_params,)* TypeOutput)> }
        };

        // Collect value types that need HasParser bounds
        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let has_parser_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens })
            .collect();

        // Collect wrapper types that need HasDialectParser bounds
        let wrapper_types = super::collect_wrapper_types(ir_input);
        let has_dialect_parser_bounds: Vec<_> = wrapper_types
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();

        // The ASTSelf type needs TypeOutput: Clone + PartialEq + 'src: 'tokens
        let all_bounds: Vec<_> = has_parser_bounds
            .into_iter()
            .chain(has_dialect_parser_bounds)
            .collect();
        let where_clause = if all_bounds.is_empty() {
            quote! { where TypeOutput: Clone + PartialEq, 'src: 'tokens }
        } else {
            quote! { where TypeOutput: Clone + PartialEq, 'src: 'tokens, #(#all_bounds),* }
        };

        // Check if we need manual trait impls:
        // - If there are wrapper variants (GAT projection bounds)
        // - If there are original type parameters (to avoid incorrect bounds on phantom data)
        let has_wrapper_variants = !wrapper_types.is_empty();
        let has_original_type_params = !type_params.is_empty();
        let needs_manual_impls = has_wrapper_variants || has_original_type_params;

        if needs_manual_impls {
            // For types with wrapper variants or original type params,
            // we need manual trait impls to avoid incorrect bounds.
            //
            // For wrapper enums, the inner AST type has wrapper variants that use GAT projections.
            // For types with original params (like T: TypeLattice), #[derive] would add T: Clone
            // even though T is only in PhantomData.
            //
            // Note: Debug impl uses a placeholder for the inner type to avoid cyclic dependency:
            // - ASTSelf: Debug would require inner AST: Debug
            // - inner AST: Debug (with LanguageOutput = ASTSelf) would require ASTSelf: Debug
            // This creates an infinite cycle. Using a placeholder breaks the cycle.
            let ast_self_name_str = ast_self_name.to_string();

            quote! {
                #[doc(hidden)]
                pub struct #ast_self_name #ast_self_def_generics (
                    pub #inner_ast_type,
                    #phantom,
                ) #where_clause;

                impl #ast_self_impl_generics Clone for #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    fn clone(&self) -> Self {
                        Self(self.0.clone(), ::core::marker::PhantomData)
                    }
                }

                impl #ast_self_impl_generics ::core::fmt::Debug for #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        // Use placeholder to avoid cyclic Debug dependency
                        // (inner AST needs LanguageOutput: Debug, which would be Self)
                        f.debug_tuple(#ast_self_name_str)
                            .field(&"..")
                            .finish()
                    }
                }

                impl #ast_self_impl_generics PartialEq for #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    fn eq(&self, other: &Self) -> bool {
                        self.0 == other.0
                    }
                }

                impl #ast_self_impl_generics #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    /// Creates a new ASTSelf wrapper.
                    pub fn new(inner: #inner_ast_type) -> Self {
                        Self(inner, ::core::marker::PhantomData)
                    }
                }
            }
        } else {
            quote! {
                #[derive(Clone, Debug, PartialEq)]
                #[doc(hidden)]
                pub struct #ast_self_name #ast_self_def_generics (
                    pub #inner_ast_type,
                    #phantom,
                ) #where_clause;

                impl #ast_self_impl_generics #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    /// Creates a new ASTSelf wrapper.
                    pub fn new(inner: #inner_ast_type) -> Self {
                        Self(inner, ::core::marker::PhantomData)
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

        // Extract original type parameters as TokenStreams
        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        // Filter to only fields needed in AST
        let mut filtered: Vec<_> = filter_ast_fields(&collected, &fields_in_fmt);

        // For tuple fields, sort by original index to match the IR field order.
        // For named fields, keep the category order (which matches iter_all_fields).
        if is_tuple {
            filtered.sort_by_key(|f| f.index);
        }

        let mut fields = Vec::new();

        for field in &filtered {
            let kind = FieldKind::from_field_info(field);
            let ty = self.field_ast_type(&field.collection, &kind, ast_name, &type_params);
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
        let crate_path = &self.config.crate_path;

        let variants: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, wrapper, .. } => {
                    // Use the associated type from HasDialectParser to ensure type compatibility.
                    // This is safe because TypeOutput and LanguageOutput don't have Dialect bounds,
                    // so there's no recursive type expansion issue.
                    let wrapped_ty = &wrapper.ty;
                    quote! {
                        #name(<#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, LanguageOutput>)
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
        type_params: &[TokenStream],
    ) -> TokenStream {
        let base = kind.ast_type(
            &self.config.crate_path,
            ast_name,
            &self.config.ir_type,
            type_params,
        );
        collection.wrap_type(base)
    }

    /// Generates manual Clone, Debug, PartialEq implementations for wrapper enums.
    ///
    /// Standard #[derive] macros don't work well with GAT projections in enum variants,
    /// so we generate manual implementations with explicit where clauses.
    #[allow(clippy::too_many_arguments)]
    fn generate_manual_trait_impls_for_wrapper_enum(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        base_bounds: &TokenStream,
        has_parser_bounds: &[TokenStream],
        _has_dialect_parser_bounds: &[TokenStream],
    ) -> TokenStream {
        use kirin_derive_core::ir::VariantRef;
        let crate_path = &self.config.crate_path;
        let ir_type = &self.config.ir_type;

        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();

        // Collect wrapper types - we only need HasDialectParser bound in base where clause,
        // not the trait-specific bounds (Clone/Debug/PartialEq)
        let wrapper_types = super::collect_wrapper_types(ir_input);
        let has_dialect_parser_base_bounds: Vec<_> = wrapper_types
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();

        // Collect value types that need Debug bounds (types containing type parameters)
        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let value_debug_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::Output: ::core::fmt::Debug })
            .collect();

        // Build base where clause without trait-specific bounds
        let where_clause = quote! {
            where
                #base_bounds
                #(#has_parser_bounds,)*
                #(#has_dialect_parser_base_bounds,)*
        };

        // Debug where clause adds Debug bounds on the actual field types
        let debug_where_clause = quote! {
            where
                #base_bounds
                #(#has_parser_bounds,)*
                #(#has_dialect_parser_base_bounds,)*
                <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output: ::core::fmt::Debug,
                LanguageOutput: ::core::fmt::Debug,
                #(#value_debug_bounds,)*
        };

        // Collect all variant names and their types for pattern matching.
        // For regular variants, we filter to only include fields that are in the AST
        // (i.e., fields in format string or fields without defaults).
        let variant_arms_clone: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, .. } => {
                    quote! {
                        #ast_name::#name(inner) => #ast_name::#name(inner.clone())
                    }
                }
                VariantRef::Regular { name, stmt } => {
                    // Get filtered AST fields (excludes default fields not in format)
                    let collected = super::collect_fields(stmt);
                    let fields_in_fmt = super::get_fields_in_format(ir_input, stmt);
                    let filtered = super::filter_ast_fields(&collected, &fields_in_fmt);

                    if stmt.is_tuple_style() {
                        let fields: Vec<_> = (0..filtered.len())
                            .map(|i| syn::Ident::new(&format!("f{}", i), proc_macro2::Span::call_site()))
                            .collect();
                        let patterns: Vec<_> = fields.iter().map(|f| quote! { #f }).collect();
                        let clones: Vec<_> = fields.iter().map(|f| quote! { #f.clone() }).collect();
                        quote! {
                            #ast_name::#name(#(#patterns,)*) => #ast_name::#name(#(#clones,)*)
                        }
                    } else {
                        let field_names: Vec<_> = filtered.iter()
                            .filter_map(|f| f.ident.as_ref())
                            .collect();
                        let clones: Vec<_> = field_names.iter().map(|f| quote! { #f: #f.clone() }).collect();
                        quote! {
                            #ast_name::#name { #(#field_names,)* } => #ast_name::#name { #(#clones,)* }
                        }
                    }
                }
            })
            .collect();

        let variant_arms_debug: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, .. } => {
                    let name_str = name.to_string();
                    // For wrapper variants, we can't require Debug on the inner GAT type
                    // because that would create circular bounds. Instead, we just print
                    // the variant name without the inner value.
                    quote! {
                        #ast_name::#name(_) => f.debug_tuple(#name_str).field(&"..").finish()
                    }
                }
                VariantRef::Regular { name, stmt } => {
                    // Get filtered AST fields (excludes default fields not in format)
                    let collected = super::collect_fields(stmt);
                    let fields_in_fmt = super::get_fields_in_format(ir_input, stmt);
                    let filtered = super::filter_ast_fields(&collected, &fields_in_fmt);

                    let name_str = name.to_string();
                    if stmt.is_tuple_style() {
                        let fields: Vec<_> = (0..filtered.len())
                            .map(|i| syn::Ident::new(&format!("f{}", i), proc_macro2::Span::call_site()))
                            .collect();
                        let patterns: Vec<_> = fields.iter().map(|f| quote! { #f }).collect();
                        let field_calls: Vec<_> = fields.iter().map(|f| quote! { .field(#f) }).collect();
                        quote! {
                            #ast_name::#name(#(#patterns,)*) => f.debug_tuple(#name_str)#(#field_calls)*.finish()
                        }
                    } else {
                        let field_names: Vec<_> = filtered.iter()
                            .filter_map(|f| f.ident.as_ref())
                            .collect();
                        let field_calls: Vec<_> = field_names.iter().map(|f| {
                            let name_str = f.to_string();
                            quote! { .field(#name_str, #f) }
                        }).collect();
                        quote! {
                            #ast_name::#name { #(#field_names,)* } => f.debug_struct(#name_str)#(#field_calls)*.finish()
                        }
                    }
                }
            })
            .collect();

        let variant_arms_eq: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, .. } => {
                    quote! {
                        (#ast_name::#name(a), #ast_name::#name(b)) => a == b
                    }
                }
                VariantRef::Regular { name, stmt } => {
                    // Get filtered AST fields (excludes default fields not in format)
                    let collected = super::collect_fields(stmt);
                    let fields_in_fmt = super::get_fields_in_format(ir_input, stmt);
                    let filtered = super::filter_ast_fields(&collected, &fields_in_fmt);

                    if stmt.is_tuple_style() {
                        let fields_a: Vec<_> = (0..filtered.len())
                            .map(|i| syn::Ident::new(&format!("a{}", i), proc_macro2::Span::call_site()))
                            .collect();
                        let fields_b: Vec<_> = (0..filtered.len())
                            .map(|i| syn::Ident::new(&format!("b{}", i), proc_macro2::Span::call_site()))
                            .collect();
                        let comparisons: Vec<_> = fields_a.iter().zip(fields_b.iter())
                            .map(|(a, b)| quote! { #a == #b })
                            .collect();
                        let comparison = if comparisons.is_empty() {
                            quote! { true }
                        } else {
                            quote! { #(#comparisons)&&* }
                        };
                        quote! {
                            (#ast_name::#name(#(#fields_a,)*), #ast_name::#name(#(#fields_b,)*)) => #comparison
                        }
                    } else {
                        let field_names: Vec<_> = filtered.iter()
                            .filter_map(|f| f.ident.as_ref())
                            .collect();
                        let fields_a: Vec<_> = field_names.iter()
                            .map(|f| syn::Ident::new(&format!("{}_a", f), f.span()))
                            .collect();
                        let fields_b: Vec<_> = field_names.iter()
                            .map(|f| syn::Ident::new(&format!("{}_b", f), f.span()))
                            .collect();
                        let patterns_a: Vec<_> = field_names.iter().zip(fields_a.iter())
                            .map(|(n, a)| quote! { #n: #a })
                            .collect();
                        let patterns_b: Vec<_> = field_names.iter().zip(fields_b.iter())
                            .map(|(n, b)| quote! { #n: #b })
                            .collect();
                        let comparisons: Vec<_> = fields_a.iter().zip(fields_b.iter())
                            .map(|(a, b)| quote! { #a == #b })
                            .collect();
                        let comparison = if comparisons.is_empty() {
                            quote! { true }
                        } else {
                            quote! { #(#comparisons)&&* }
                        };
                        quote! {
                            (#ast_name::#name { #(#patterns_a,)* }, #ast_name::#name { #(#patterns_b,)* }) => #comparison
                        }
                    }
                }
            })
            .collect();

        // Generate additional bounds for traits
        // Clone and PartialEq bounds are needed for wrapper variants.
        // Debug does NOT need bounds because we print a placeholder for wrapper variants.
        let wrapper_types = super::collect_wrapper_types(ir_input);
        let clone_bounds: Vec<_> = wrapper_types.iter()
            .map(|ty| quote! { <#ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, LanguageOutput>: Clone })
            .collect();
        let partial_eq_bounds: Vec<_> = wrapper_types.iter()
            .map(|ty| quote! { <#ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, LanguageOutput>: PartialEq })
            .collect();

        quote! {
            impl #impl_generics Clone for #ast_name #ty_generics
            #where_clause
                #(#clone_bounds,)*
            {
                fn clone(&self) -> Self {
                    match self {
                        #(#variant_arms_clone,)*
                        #ast_name::__Marker(_, unreachable) => match *unreachable {},
                    }
                }
            }

            impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
            #debug_where_clause
            {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        #(#variant_arms_debug,)*
                        #ast_name::__Marker(_, unreachable) => match *unreachable {},
                    }
                }
            }

            impl #impl_generics PartialEq for #ast_name #ty_generics
            #where_clause
                #(#partial_eq_bounds,)*
            {
                fn eq(&self, other: &Self) -> bool {
                    match (self, other) {
                        #(#variant_arms_eq,)*
                        (#ast_name::__Marker(_, unreachable), _) => match *unreachable {},
                        (_, #ast_name::__Marker(_, unreachable)) => match *unreachable {},
                        _ => false,
                    }
                }
            }
        }
    }
}
