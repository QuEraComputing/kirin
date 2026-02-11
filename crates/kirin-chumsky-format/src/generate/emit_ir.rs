//! Code generation for the `EmitIR` derive macro.

use std::collections::HashSet;

use kirin_derive_core::ir::{
    VariantRef,
    fields::{FieldCategory, FieldInfo},
};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::field_kind::{FieldKind, collect_fields};

use super::{
    BoundsBuilder, GeneratorConfig, collect_all_value_types_needing_bounds, filter_ast_fields,
    generate_enum_match, get_fields_in_format,
};

/// Generator for the `EmitIR` trait implementation.
pub struct GenerateEmitIR {
    config: GeneratorConfig,
}

impl GenerateEmitIR {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        Self {
            config: GeneratorConfig::new(ir_input),
        }
    }

    /// Generates the `EmitIR` implementation.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        // For wrapper structs, the AST type is a type alias to the wrapped type's AST.
        // The wrapped type's AST already implements EmitIR, so no impl is needed.
        if let kirin_derive_core::ir::Data::Struct(data) = &ir_input.data {
            if data.0.wraps.is_some() {
                return TokenStream::new();
            }
        }

        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_self_name =
            syn::Ident::new(&format!("{}ASTSelf", ir_input.name), ir_input.name.span());
        let ast_generics = super::build_ast_generics(&ir_input.generics, true);
        let crate_path = &self.config.crate_path;

        // Generate impl for the AST type
        let emit_impl = self.generate_emit_impl(ir_input, &ast_name, &ast_generics, crate_path);

        // Generate impl for the ASTSelf wrapper - delegates to inner type
        let ast_self_emit_impl =
            self.generate_ast_self_emit_impl(ir_input, &ast_name, &ast_self_name, crate_path);

        quote! {
            #emit_impl
            #ast_self_emit_impl
        }
    }

    /// Builds just the type generics for the AST type (without Language).
    ///
    /// Returns a TokenStream like `<'tokens, 'src, [original type params], TypeOutput, LanguageOutput>`.
    fn build_ast_ty_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        // Extract original type parameters
        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput, LanguageOutput> }
        } else {
            quote! { <'tokens, 'src, #(#type_params,)* TypeOutput, LanguageOutput> }
        }
    }

    fn language_output_emit_bound(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        crate_path: &syn::Path,
        ir_path: &syn::Path,
    ) -> TokenStream {
        if self.ast_needs_language_output_emit_bound(ir_input) {
            quote! {
                LanguageOutput: #crate_path::EmitIR<Language, Output = #ir_path::Statement>,
            }
        } else {
            TokenStream::new()
        }
    }

    fn ast_needs_language_output_emit_bound(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> bool {
        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(data) => {
                self.statement_needs_language_output_emit_bound(ir_input, &data.0)
            }
            kirin_derive_core::ir::Data::Enum(data) => {
                data.iter_variants().any(|variant| match variant {
                    VariantRef::Wrapper { .. } => false,
                    VariantRef::Regular { stmt, .. } => {
                        self.statement_needs_language_output_emit_bound(ir_input, stmt)
                    }
                })
            }
        }
    }

    fn statement_needs_language_output_emit_bound(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
    ) -> bool {
        if stmt.wraps.is_some() {
            return false;
        }

        // Fast path: if the statement has no block/region fields at all, AST filtering
        // cannot produce recursive statement output requirements.
        if !self.statement_contains_statement_recursion_fields(stmt) {
            return false;
        }

        let collected = collect_fields(stmt);
        let fields_in_fmt = get_fields_in_format(ir_input, stmt);
        let ast_fields = filter_ast_fields(&collected, &fields_in_fmt);
        self.ast_fields_contain_statement_recursion_fields(&ast_fields)
    }

    fn statement_contains_statement_recursion_fields(
        &self,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
    ) -> bool {
        stmt.iter_all_fields().any(|field| {
            matches!(
                field.category(),
                FieldCategory::Block | FieldCategory::Region
            )
        })
    }

    fn ast_fields_contain_statement_recursion_fields(
        &self,
        ast_fields: &[&FieldInfo<ChumskyLayout>],
    ) -> bool {
        ast_fields.iter().any(|field| {
            matches!(
                field.category(),
                FieldCategory::Block | FieldCategory::Region
            )
        })
    }

    fn is_ir_type_a_type_param(&self, ir_type: &syn::Path, generics: &syn::Generics) -> bool {
        // Type parameter must be a single segment path (e.g., `T`, not `foo::T`)
        if ir_type.segments.len() != 1 {
            return false;
        }

        let ir_type_name = &ir_type.segments[0].ident;

        // Check if this matches any of the struct's type parameters
        generics.type_params().any(|tp| &tp.ident == ir_type_name)
    }

    fn generate_emit_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        // ast_generics includes Language, but the AST type doesn't have Language
        // So we use ast_generics for impl_generics, but build ty_generics without Language
        let (impl_generics, _, _) = ast_generics.split_for_impl();

        // Build ty_generics without the Language parameter
        let ty_generics = self.build_ast_ty_generics(ir_input);

        let (_, original_ty_generics, _) = ir_input.generics.split_for_impl();

        let emit_body = match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(s) => self.generate_struct_emit(
                ir_input,
                &s.0,
                original_name,
                &original_ty_generics,
                None,
            ),
            kirin_derive_core::ir::Data::Enum(e) => {
                self.generate_enum_emit(ir_input, e, original_name, &original_ty_generics, ast_name)
            }
        };

        let ir_type = &self.config.ir_type;
        let ir_path = &self.config.ir_path;

        // Use BoundsBuilder to generate EmitIR bounds
        let bounds = BoundsBuilder::new(crate_path, ir_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.emit_ir_bounds(&value_types);

        // For wrapper enum variants, we need:
        // - Language: From<WrappedDialect> for each wrapped type (so inner.emit(ctx) works)
        // - The wrapped type's AST (with TypeOutput and LanguageOutput) must implement EmitIR
        let wrapper_types = super::collect_wrapper_types(ir_input);
        let wrapper_from_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| syn::parse_quote! { Language: ::core::convert::From<#ty> })
            .collect();
        // With new design, wrapper variants use the direct AST type name
        // (e.g., WrappedDialectAST<..., TypeOutput, LanguageOutput>)
        // which is already in our AST's variant, so it will naturally implement EmitIR
        // if we have the proper bounds.
        let wrapper_emit_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| {
                // Use new syntax: Output<TypeOutput, LanguageOutput>
                syn::parse_quote! {
                    <#ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, LanguageOutput>:
                        #crate_path::EmitIR<Language, Output = #ir_path::Statement>
                }
            })
            .collect();
        let wrapper_dialect_parser_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();

        // IR type parameter for the EmitIR impl
        // We need:
        // - `From<OriginalType>` to convert the AST to IR statements
        // - <ir_type as HasParser>::Output must implement EmitIR to convert parsed type annotations
        //   to Dialect::Type (this is the actual type in the AST fields)
        // - ir_type: HasParser + 'tokens bound
        // - TypeOutput: Clone + PartialEq (from AST bounds, used in PhantomData)
        // - LanguageOutput: Clone + PartialEq + 'tokens + EmitIR<Language> (for Block/Region recursion)
        // - For Value field types with type parameters: HasParser + EmitIR bounds
        // - For wrapper variants: Language: From<WrappedType> and inner AST: EmitIR<Language>
        //
        // If ir_type is a type parameter (e.g., `T`), we also need to add an explicit
        // `Language: Dialect<Type = T>` bound. This ensures consistency between:
        // - `<T as HasParser>::Output: EmitIR<Language, Output = Language::Type>`
        // - `<T as HasParser>::Output: EmitIR<Language, Output = T>` (from value type bounds)
        // Without this, the compiler can't prove that `Language::Type == T`.
        let ir_type_is_param = self.is_ir_type_a_type_param(ir_type, &ir_input.generics);
        let dialect_type_bound = if ir_type_is_param {
            quote! { Language: #ir_path::Dialect<Type = #ir_type>, }
        } else {
            quote! {}
        };
        let language_output_emit_bound =
            self.language_output_emit_bound(ir_input, crate_path, ir_path);
        let base_bounds = quote! {
            Language: #ir_path::Dialect + From<#original_name #original_ty_generics>,
            #dialect_type_bound
            TypeOutput: Clone + PartialEq,
            LanguageOutput: Clone + PartialEq + 'tokens,
            #language_output_emit_bound
            #ir_type: #crate_path::HasParser<'tokens, 'src> + 'tokens,
            <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output:
                #crate_path::EmitIR<Language, Output = <Language as #ir_path::Dialect>::Type>,
        };

        let mut all_bounds = vec![base_bounds];
        if !value_type_bounds.is_empty() {
            let bounds_tokens = value_type_bounds.iter().map(|b| quote! { #b, });
            all_bounds.push(quote! { #(#bounds_tokens)* });
        }
        if !wrapper_from_bounds.is_empty() {
            let from_tokens = wrapper_from_bounds.iter().map(|b| quote! { #b, });
            let emit_tokens = wrapper_emit_bounds.iter().map(|b| quote! { #b, });
            let dialect_parser_tokens =
                wrapper_dialect_parser_bounds.iter().map(|b| quote! { #b, });
            all_bounds
                .push(quote! { #(#from_tokens)* #(#emit_tokens)* #(#dialect_parser_tokens)* });
        }

        let where_clause = quote! { where #(#all_bounds)* };

        quote! {
            impl #impl_generics #crate_path::EmitIR<Language> for #ast_name #ty_generics
            #where_clause
            {
                type Output = #ir_path::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> Self::Output {
                    #emit_body
                }
            }
        }
    }

    /// Generates an EmitIR impl for the ASTSelf wrapper that delegates to the inner type.
    fn generate_ast_self_emit_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_self_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_path = &self.config.ir_path;
        let ir_type = &self.config.ir_type;
        let (_, original_ty_generics, _) = ir_input.generics.split_for_impl();

        // Extract original type parameters
        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        // Build generics for the ASTSelf impl: <'tokens, 'src, [original type params], TypeOutput, Language>
        let impl_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput, Language> }
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
            quote! { <'tokens, 'src, #(#type_param_bounds,)* TypeOutput, Language> }
        };

        // Build type generics for ASTSelf: <'tokens, 'src, [params], TypeOutput>
        let ast_self_ty_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput> }
        } else {
            quote! { <'tokens, 'src, #(#type_params,)* TypeOutput> }
        };

        // Note: We do NOT use inner_ast_type in bounds to avoid recursive trait resolution.
        // The bound `inner_ast_type: EmitIR<Language>` would cause overflow.
        let _inner_ast_type = if type_params.is_empty() {
            quote! { #ast_name<'tokens, 'src, TypeOutput, #ast_self_name<'tokens, 'src, TypeOutput>> }
        } else {
            quote! { #ast_name<'tokens, 'src, #(#type_params,)* TypeOutput, #ast_self_name<'tokens, 'src, #(#type_params,)* TypeOutput>> }
        };

        // Use BoundsBuilder to generate EmitIR bounds
        let bounds = BoundsBuilder::new(crate_path, ir_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.emit_ir_bounds(&value_types);

        // Collect wrapper types for their bounds
        let wrapper_types = super::collect_wrapper_types(ir_input);
        let wrapper_from_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| syn::parse_quote! { Language: ::core::convert::From<#ty> })
            .collect();
        let wrapper_dialect_parser_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();
        // For wrapper enums, we need the inner ASTs to implement EmitIR.
        // The LanguageOutput for these is Self (the ASTSelf type), creating a coinductive requirement.
        // We use the ASTSelf type directly to allow Rust's trait solver to find the coinductive solution.
        let wrapper_emit_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| {
                syn::parse_quote! {
                    <#ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, #ast_self_name #ast_self_ty_generics>:
                        #crate_path::EmitIR<Language, Output = #ir_path::Statement>
                }
            })
            .collect();

        // Note: We do NOT add a bound requiring `inner_ast_type: EmitIR<Language>` because
        // that would create a recursive bound (ASTSelf contains AST which contains ASTSelf...).
        // Instead, we rely on the wrapper_emit_bounds which use coinduction.
        //
        // If ir_type is a type parameter, add explicit Language::Type bound
        // (same reasoning as in generate_emit_impl).
        let ir_type_is_param = self.is_ir_type_a_type_param(ir_type, &ir_input.generics);
        let dialect_type_bound = if ir_type_is_param {
            quote! { Language: #ir_path::Dialect<Type = #ir_type>, }
        } else {
            quote! {}
        };
        let base_bounds = quote! {
            Language: #ir_path::Dialect + From<#original_name #original_ty_generics>,
            #dialect_type_bound
            TypeOutput: Clone + PartialEq,
            'src: 'tokens,
            #ir_type: #crate_path::HasParser<'tokens, 'src> + 'tokens,
            <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output:
                #crate_path::EmitIR<Language, Output = <Language as #ir_path::Dialect>::Type>,
        };

        let mut all_bounds = vec![base_bounds];
        if !value_type_bounds.is_empty() {
            let bounds_tokens = value_type_bounds.iter().map(|b| quote! { #b, });
            all_bounds.push(quote! { #(#bounds_tokens)* });
        }
        if !wrapper_from_bounds.is_empty() {
            let from_tokens = wrapper_from_bounds.iter().map(|b| quote! { #b, });
            let emit_tokens = wrapper_emit_bounds.iter().map(|b| quote! { #b, });
            let dialect_parser_tokens =
                wrapper_dialect_parser_bounds.iter().map(|b| quote! { #b, });
            all_bounds
                .push(quote! { #(#from_tokens)* #(#emit_tokens)* #(#dialect_parser_tokens)* });
        }

        let where_clause = quote! { where #(#all_bounds)* };

        quote! {
            impl #impl_generics #crate_path::EmitIR<Language> for #ast_self_name #ast_self_ty_generics
            #where_clause
            {
                type Output = #ir_path::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> Self::Output {
                    // Delegate to the inner AST type - use fully qualified syntax
                    #crate_path::EmitIR::emit(&self.0, ctx)
                }
            }
        }
    }

    fn generate_struct_emit(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        original_name: &syn::Ident,
        original_ty_generics: &syn::TypeGenerics<'_>,
        variant_name: Option<&syn::Ident>,
    ) -> TokenStream {
        let collected = collect_fields(stmt);
        let fields_in_fmt = get_fields_in_format(ir_input, stmt);
        let ast_fields = filter_ast_fields(&collected, &fields_in_fmt);

        let (pattern, emit_calls, constructor) = self.build_emit_components(
            ir_input,
            stmt,
            original_name,
            variant_name,
            &collected,
            &ast_fields,
            &fields_in_fmt,
            true, // is_struct (use Self pattern)
        );

        quote! {
            let #pattern = self;
            #emit_calls
            let dialect_variant: #original_name #original_ty_generics = #constructor;
            ctx.stage.statement().definition(dialect_variant).new()
        }
    }

    /// Builds the pattern, emit calls, and constructor for a statement.
    ///
    /// This is shared between struct and variant emit generation.
    fn build_emit_components(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        original_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
        collected: &[FieldInfo<ChumskyLayout>],
        ast_fields: &[&FieldInfo<ChumskyLayout>],
        fields_in_fmt: &std::collections::HashSet<usize>,
        is_struct: bool,
    ) -> (TokenStream, TokenStream, TokenStream) {
        let is_tuple = stmt.is_tuple_style();

        if is_tuple {
            let mut sorted_ast_fields: Vec<_> = ast_fields.to_vec();
            sorted_ast_fields.sort_by_key(|f| f.index);

            let field_vars: Vec<_> = sorted_ast_fields
                .iter()
                .map(|f| syn::Ident::new(&format!("f{}", f.index), proc_macro2::Span::call_site()))
                .collect();

            let pattern = if is_struct {
                quote! { Self(#(#field_vars),*) }
            } else {
                quote! { #(#field_vars),* }
            };

            let emit_calls = self.generate_field_emit_calls(
                &sorted_ast_fields,
                &field_vars,
                &ir_input.generics,
                true,
            );

            let constructor = self.generate_dialect_constructor_with_defaults(
                original_name,
                variant_name,
                collected,
                &sorted_ast_fields,
                &field_vars,
                fields_in_fmt,
                true,
            );

            (pattern, emit_calls, constructor)
        } else {
            let field_vars: Vec<_> = ast_fields
                .iter()
                .map(|f| {
                    let ident = f.ident.as_ref().unwrap();
                    syn::Ident::new(&format!("f_{}", ident), proc_macro2::Span::call_site())
                })
                .collect();

            let pat: Vec<_> = ast_fields
                .iter()
                .zip(&field_vars)
                .map(|(f, b)| {
                    let orig = f.ident.as_ref().unwrap();
                    quote! { #orig: #b }
                })
                .collect();

            let pattern = if is_struct {
                // Use `..` to ignore the hidden `_marker` field in the AST
                quote! { Self { #(#pat,)* .. } }
            } else {
                quote! { #(#pat),* }
            };

            let emit_calls =
                self.generate_field_emit_calls(ast_fields, &field_vars, &ir_input.generics, false);

            let constructor = self.generate_dialect_constructor_with_defaults(
                original_name,
                variant_name,
                collected,
                ast_fields,
                &field_vars,
                fields_in_fmt,
                false,
            );

            (pattern, emit_calls, constructor)
        }
    }

    fn generate_enum_emit(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
        original_name: &syn::Ident,
        original_ty_generics: &syn::TypeGenerics<'_>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let marker = quote! {
            #ast_name::__Marker(_, unreachable) => match *unreachable {}
        };

        let crate_path = &self.config.crate_path;
        let crate_path_for_match = crate_path.clone();
        generate_enum_match(
            ast_name,
            data,
            // Wrapper handler - use fully qualified syntax to ensure EmitIR is in scope
            move |_name, _wrapper| {
                quote! { #crate_path_for_match::EmitIR::emit(inner, ctx) }
            },
            // Regular variant handler
            |name, variant| {
                self.generate_variant_emit(
                    ir_input,
                    variant,
                    original_name,
                    original_ty_generics,
                    ast_name,
                    name,
                )
            },
            Some(marker),
        )
    }

    /// Generates emit code for a single enum variant.
    fn generate_variant_emit(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        variant: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        original_name: &syn::Ident,
        original_ty_generics: &syn::TypeGenerics<'_>,
        ast_name: &syn::Ident,
        variant_name: &syn::Ident,
    ) -> TokenStream {
        let collected = collect_fields(variant);
        let fields_in_fmt = get_fields_in_format(ir_input, variant);
        let ast_fields = filter_ast_fields(&collected, &fields_in_fmt);
        let is_tuple = variant.is_tuple_style();

        let (pattern, emit_calls, constructor) = self.build_emit_components(
            ir_input,
            variant,
            original_name,
            Some(variant_name),
            &collected,
            &ast_fields,
            &fields_in_fmt,
            false, // not a struct, it's a variant
        );

        // Build the match arm pattern with the AST name
        let full_pattern = if ast_fields.is_empty() {
            if is_tuple {
                quote! { #ast_name::#variant_name }
            } else {
                quote! { #ast_name::#variant_name {} }
            }
        } else if is_tuple {
            quote! { #ast_name::#variant_name(#pattern) }
        } else {
            quote! { #ast_name::#variant_name { #pattern } }
        };

        quote! {
            #full_pattern => {
                #emit_calls
                let dialect_variant: #original_name #original_ty_generics = #constructor;
                ctx.stage.statement().definition(dialect_variant).new()
            }
        }
    }

    fn generate_field_emit_calls(
        &self,
        ast_fields: &[&FieldInfo<ChumskyLayout>],
        field_vars: &[syn::Ident],
        generics: &syn::Generics,
        _is_tuple: bool,
    ) -> TokenStream {
        let crate_path = &self.config.crate_path;
        let ir_path = &self.config.ir_path;

        // Get type parameter names for checking if a Value type needs EmitIR::emit
        let type_param_names: Vec<String> = generics
            .type_params()
            .map(|p| p.ident.to_string())
            .collect();

        // ast_fields and field_vars should already be in the correct order
        let emit_stmts: Vec<_> = ast_fields
            .iter()
            .zip(field_vars.iter())
            .map(|(field, var)| {
                let emitted_var = syn::Ident::new(
                    &format!("{}_ir", var),
                    proc_macro2::Span::call_site(),
                );

                // Use FieldKind to determine the emit behavior
                let kind = FieldKind::from_field_info(field);
                match kind {
                    FieldKind::SSAValue => {
                        quote! {
                            let #emitted_var: #ir_path::SSAValue = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::ResultValue => {
                        quote! {
                            let #emitted_var: #ir_path::ResultValue = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::Block => {
                        quote! {
                            let #emitted_var: #ir_path::Block = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::Successor => {
                        quote! {
                            let #emitted_var: #ir_path::Successor = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::Region => {
                        quote! {
                            let #emitted_var: #ir_path::Region = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::Symbol => {
                        quote! {
                            let #emitted_var: #ir_path::Symbol = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::Value(ref ty) => {
                        // Check if this Value type contains any type parameters
                        let needs_emit_ir = type_param_names.iter().any(|param_name| {
                            kirin_derive_core::misc::is_type(ty, param_name.as_str())
                                || kirin_derive_core::misc::is_type_in_generic(ty, param_name.as_str())
                        });

                        if needs_emit_ir {
                            // For Value types containing type parameters, call EmitIR::emit
                            // to convert from the AST representation to the IR representation
                            quote! {
                                let #emitted_var = #crate_path::EmitIR::emit(#var, ctx);
                            }
                        } else {
                            // For concrete Value types, just clone directly
                            // (the AST type equals the IR type via HasParser<Output = T>)
                            quote! {
                                let #emitted_var = #var.clone();
                            }
                        }
                    }
                }
            })
            .collect();

        quote! {
            #(#emit_stmts)*
        }
    }

    /// Generates the dialect constructor, handling both AST fields and fields with defaults.
    ///
    /// - `all_fields`: All fields in the original struct/variant
    /// - `ast_fields`: Fields that are in the AST (parsed from format string)
    /// - `field_vars`: Variable names for the AST fields (in same order as ast_fields)
    /// - `fields_in_fmt`: Set of field indices that are in the format string
    fn generate_dialect_constructor_with_defaults(
        &self,
        original_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
        all_fields: &[FieldInfo<ChumskyLayout>],
        ast_fields: &[&FieldInfo<ChumskyLayout>],
        field_vars: &[syn::Ident],
        _fields_in_fmt: &HashSet<usize>,
        is_tuple: bool,
    ) -> TokenStream {
        // Build a map from field index to variable name for AST fields
        let ast_field_vars: std::collections::HashMap<usize, &syn::Ident> = ast_fields
            .iter()
            .zip(field_vars.iter())
            .map(|(f, v)| (f.index, v))
            .collect();

        // Sort all_fields by index for tuple types
        let ordered_all_fields: Vec<_> = if is_tuple {
            let mut sorted: Vec<_> = all_fields.iter().collect();
            sorted.sort_by_key(|f| f.index);
            sorted
        } else {
            all_fields.iter().collect()
        };

        // Generate the field values for the constructor
        let field_values: Vec<_> = ordered_all_fields
            .iter()
            .map(|field| {
                // Check if this field is in the AST (was parsed)
                if let Some(var) = ast_field_vars.get(&field.index) {
                    let emitted_var =
                        syn::Ident::new(&format!("{}_ir", var), proc_macro2::Span::call_site());

                    // Field was parsed - use the emitted value
                    // Use category to determine if we need .into()
                    match field.category() {
                        FieldCategory::Argument
                        | FieldCategory::Result
                        | FieldCategory::Block
                        | FieldCategory::Successor
                        | FieldCategory::Region
                        | FieldCategory::Symbol => {
                            quote! { #emitted_var.into() }
                        }
                        FieldCategory::Value => {
                            quote! { #emitted_var }
                        }
                    }
                } else if let Some(default_value) = field.default_value() {
                    // Field has a default - use the default expression
                    let default_expr = default_value.to_expr();
                    quote! { #default_expr }
                } else {
                    // This shouldn't happen - validation should have caught it
                    // But as a fallback, use Default::default()
                    quote! { ::core::default::Default::default() }
                }
            })
            .collect();

        if is_tuple {
            match variant_name {
                Some(v) => quote! { #original_name::#v(#(#field_values),*) },
                None => quote! { #original_name(#(#field_values),*) },
            }
        } else {
            let field_assigns: Vec<_> = ordered_all_fields
                .iter()
                .zip(field_values.iter())
                .map(|(field, value)| {
                    let name = field.ident.as_ref().unwrap();
                    quote! { #name: #value }
                })
                .collect();

            match variant_name {
                Some(v) => quote! { #original_name::#v { #(#field_assigns),* } },
                None => quote! { #original_name { #(#field_assigns),* } },
            }
        }
    }
}
