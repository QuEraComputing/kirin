//! Code generation for the `EmitIR` derive macro.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{CollectedField, FieldKind, collect_fields, fields_in_format};
use crate::format::Format;
use crate::generics::GenericsBuilder;

/// Generator for the `EmitIR` trait implementation.
pub struct GenerateEmitIR {
    crate_path: syn::Path,
    ir_path: syn::Path,
    type_lattice: syn::Path,
}

impl GenerateEmitIR {
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

    /// Generates the `EmitIR` implementation.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_generics = self.build_ast_generics(ir_input);
        let crate_path = &self.crate_path;

        // Generate impl for the AST type
        let emit_impl = self.generate_emit_impl(ir_input, &ast_name, &ast_generics, crate_path);

        quote! {
            #emit_impl
        }
    }

    fn build_ast_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.ir_path).with_language(&ir_input.generics)
    }

    fn generate_emit_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();
        let (_, original_ty_generics, _) = ir_input.generics.split_for_impl();

        let emit_body = match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(s) => {
                self.generate_struct_emit(ir_input, &s.0, original_name, None)
            }
            kirin_derive_core::ir::Data::Enum(e) => {
                self.generate_enum_emit(ir_input, e, original_name, ast_name)
            }
        };

        let type_lattice = &self.type_lattice;
        let ir_path = &self.ir_path;

        // IR type parameter for the EmitIR impl
        // We need:
        // - `From<OriginalType>` to convert the AST to IR statements
        // - TypeAST (= <type_lattice as HasParser>::Output) must implement EmitIR to convert
        //   parsed type annotations to TypeLattice
        // - The TypeLatticeEmit bound on HasParser::Output provides EmitIR when Output = TypeLattice
        quote! {
            impl #impl_generics #crate_path::EmitIR<Language> for #ast_name #ty_generics
            where
                Language: #ir_path::Dialect + From<#original_name #original_ty_generics>,
                <#type_lattice as #crate_path::HasParser<'tokens, 'src>>::Output: #crate_path::EmitIR<Language, Output = <Language as #ir_path::Dialect>::TypeLattice>,
            {
                type Output = #ir_path::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> Self::Output {
                    #emit_body
                }
            }
        }
    }

    fn generate_struct_emit(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        original_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
    ) -> TokenStream {
        let collected = collect_fields(stmt);
        let fields_in_fmt = self.get_fields_in_format(ir_input, stmt);

        // Filter to only fields that are in the AST (in format or required)
        let ast_fields: Vec<_> = collected
            .iter()
            .filter(|f| fields_in_fmt.contains(&f.index) || f.default.is_none())
            .collect();

        // Generate field bindings only for AST fields
        let is_tuple = stmt.is_tuple_style();

        let (pattern, emit_calls, constructor) = if is_tuple {
            // For tuple fields, we need to handle the pattern carefully
            // AST only contains fields that are in format or don't have defaults
            let mut sorted_ast_fields = ast_fields.clone();
            sorted_ast_fields.sort_by_key(|f| f.index);

            let field_vars: Vec<_> = sorted_ast_fields
                .iter()
                .map(|f| {
                    syn::Ident::new(&format!("f{}", f.index), proc_macro2::Span::call_site())
                })
                .collect();

            let pattern = quote! { Self(#(#field_vars),*) };

            // Generate emit calls for AST fields
            let emit_calls =
                self.generate_field_emit_calls(&sorted_ast_fields, &field_vars, true);

            // Generate dialect constructor using all fields (AST + defaults)
            let constructor = self.generate_dialect_constructor_with_defaults(
                original_name,
                variant_name,
                &collected,
                &sorted_ast_fields,
                &field_vars,
                &fields_in_fmt,
                true,
            );

            (pattern, emit_calls, constructor)
        } else {
            // For named fields, bind only AST fields
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
            // Use `..` to ignore the hidden `_marker` field in the AST
            let pattern = quote! { Self { #(#pat,)* .. } };

            // Generate emit calls for AST fields
            let emit_calls = self.generate_field_emit_calls(&ast_fields, &field_vars, false);

            // Generate dialect constructor using all fields (AST + defaults)
            let constructor = self.generate_dialect_constructor_with_defaults(
                original_name,
                variant_name,
                &collected,
                &ast_fields,
                &field_vars,
                &fields_in_fmt,
                false,
            );

            (pattern, emit_calls, constructor)
        };

        quote! {
            let #pattern = self;
            #emit_calls
            let dialect_variant: #original_name = #constructor;
            ctx.context.statement().definition(dialect_variant).new()
        }
    }

    fn generate_enum_emit(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
        original_name: &syn::Ident,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let arms: Vec<_> = data
            .variants
            .iter()
            .map(|variant| {
                let name = &variant.name;

                // Check if this is a wrapper variant
                if let Some(_wrapper) = &variant.wraps {
                    // For wrapper variants, the inner value should implement EmitIR
                    return quote! {
                        #ast_name::#name(inner) => {
                            inner.emit(ctx)
                        }
                    };
                }

                let collected = collect_fields(variant);
                let fields_in_fmt = self.get_fields_in_format(ir_input, variant);

                // Filter to only fields that are in the AST (in format or required)
                let ast_fields: Vec<_> = collected
                    .iter()
                    .filter(|f| fields_in_fmt.contains(&f.index) || f.default.is_none())
                    .collect();

                let is_tuple = variant.is_tuple_style();

                if ast_fields.is_empty() {
                    // All fields have defaults or no fields at all
                    let constructor = self.generate_dialect_constructor_with_defaults(
                        original_name,
                        Some(name),
                        &collected,
                        &[],
                        &[],
                        &fields_in_fmt,
                        is_tuple,
                    );
                    if is_tuple {
                        quote! {
                            #ast_name::#name => {
                                let dialect_variant: #original_name = #constructor;
                                ctx.context.statement().definition(dialect_variant).new()
                            }
                        }
                    } else {
                        quote! {
                            #ast_name::#name {} => {
                                let dialect_variant: #original_name = #constructor;
                                ctx.context.statement().definition(dialect_variant).new()
                            }
                        }
                    }
                } else if is_tuple {
                    let mut sorted_ast_fields = ast_fields.clone();
                    sorted_ast_fields.sort_by_key(|f| f.index);

                    let field_vars: Vec<_> = sorted_ast_fields
                        .iter()
                        .map(|f| {
                            syn::Ident::new(&format!("f{}", f.index), proc_macro2::Span::call_site())
                        })
                        .collect();

                    let emit_calls =
                        self.generate_field_emit_calls(&sorted_ast_fields, &field_vars, true);
                    let constructor = self.generate_dialect_constructor_with_defaults(
                        original_name,
                        Some(name),
                        &collected,
                        &sorted_ast_fields,
                        &field_vars,
                        &fields_in_fmt,
                        true,
                    );
                    quote! {
                        #ast_name::#name(#(#field_vars),*) => {
                            #emit_calls
                            let dialect_variant: #original_name = #constructor;
                            ctx.context.statement().definition(dialect_variant).new()
                        }
                    }
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
                    let emit_calls = self.generate_field_emit_calls(&ast_fields, &field_vars, false);
                    let constructor = self.generate_dialect_constructor_with_defaults(
                        original_name,
                        Some(name),
                        &collected,
                        &ast_fields,
                        &field_vars,
                        &fields_in_fmt,
                        false,
                    );
                    quote! {
                        #ast_name::#name { #(#pat),* } => {
                            #emit_calls
                            let dialect_variant: #original_name = #constructor;
                            ctx.context.statement().definition(dialect_variant).new()
                        }
                    }
                }
            })
            .collect();

        // Add handler for __Marker variant (uninhabited, so unreachable)
        quote! {
            match self {
                #(#arms)*
                #ast_name::__Marker(_, unreachable) => match *unreachable {},
            }
        }
    }

    fn generate_field_emit_calls(
        &self,
        ast_fields: &[&CollectedField],
        field_vars: &[syn::Ident],
        _is_tuple: bool,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let ir_path = &self.ir_path;

        // ast_fields and field_vars should already be in the correct order
        let emit_stmts: Vec<_> = ast_fields
            .iter()
            .zip(field_vars.iter())
            .map(|(field, var)| {
                let emitted_var = syn::Ident::new(
                    &format!("{}_ir", var),
                    proc_macro2::Span::call_site(),
                );

                match &field.kind {
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
                    FieldKind::Value(_ty) => {
                        // For compile-time values, we clone them directly
                        // (they should be the same type in AST and IR)
                        quote! {
                            let #emitted_var = #var.clone();
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
        all_fields: &[CollectedField],
        ast_fields: &[&CollectedField],
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
                    match &field.kind {
                        FieldKind::SSAValue
                        | FieldKind::ResultValue
                        | FieldKind::Block
                        | FieldKind::Successor
                        | FieldKind::Region => {
                            quote! { #emitted_var.into() }
                        }
                        FieldKind::Value(_) => {
                            quote! { #emitted_var }
                        }
                    }
                } else if let Some(default_value) = &field.default {
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
