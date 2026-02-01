//! Code generation for the `EmitIR` derive macro.

use proc_macro2::TokenStream;
use quote::quote;

use crate::field_kind::{CollectedField, FieldKind, collect_fields};
use crate::generics::GenericsBuilder;
use crate::ChumskyLayout;

/// Generator for the `EmitIR` trait implementation.
pub struct GenerateEmitIR {
    crate_path: syn::Path,
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
        Self { crate_path }
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
        GenericsBuilder::new(&self.crate_path).with_language(&ir_input.generics)
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
                self.generate_struct_emit(&s.0, original_name, None)
            }
            kirin_derive_core::ir::Data::Enum(e) => {
                self.generate_enum_emit(e, original_name, ast_name)
            }
        };

        // IR type parameter for the EmitIR impl
        // We need an additional type parameter that satisfies `Dialect + From<OriginalType>`
        quote! {
            impl #impl_generics #crate_path::EmitIR<Language> for #ast_name #ty_generics
            where
                Language: ::kirin_ir::Dialect + From<#original_name #original_ty_generics> + #crate_path::LanguageParser<'tokens, 'src>,
            {
                type Output = ::kirin_ir::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> Self::Output {
                    #emit_body
                }
            }
        }
    }

    fn generate_struct_emit(
        &self,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        original_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
    ) -> TokenStream {
        let collected = collect_fields(stmt);

        // Generate field bindings and emit calls
        let bindings = stmt.field_bindings("f");
        let fields = &bindings.field_idents;

        let (pattern, emit_calls, constructor) = if bindings.is_tuple {
            let pattern = quote! { Self(#(#fields),*) };

            // Generate emit calls for each field
            let emit_calls = self.generate_field_emit_calls(&collected, fields);

            // Generate dialect constructor
            let constructor = self.generate_dialect_constructor(
                original_name,
                variant_name,
                &collected,
                fields,
                true,
            );

            (pattern, emit_calls, constructor)
        } else {
            let orig_fields = &bindings.original_field_names;
            let pat: Vec<_> = orig_fields
                .iter()
                .zip(fields)
                .map(|(f, b)| quote! { #f: #b })
                .collect();
            let pattern = quote! { Self { #(#pat),* } };

            // Generate emit calls for each field
            let emit_calls = self.generate_field_emit_calls(&collected, fields);

            // Generate dialect constructor
            let constructor = self.generate_dialect_constructor(
                original_name,
                variant_name,
                &collected,
                fields,
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
                let bindings = variant.field_bindings("f");
                let fields = &bindings.field_idents;

                if bindings.is_empty() {
                    if bindings.is_tuple {
                        let constructor = quote! { #original_name::#name };
                        quote! {
                            #ast_name::#name => {
                                let dialect_variant: #original_name = #constructor;
                                ctx.context.statement().definition(dialect_variant).new()
                            }
                        }
                    } else {
                        let constructor = quote! { #original_name::#name {} };
                        quote! {
                            #ast_name::#name {} => {
                                let dialect_variant: #original_name = #constructor;
                                ctx.context.statement().definition(dialect_variant).new()
                            }
                        }
                    }
                } else if bindings.is_tuple {
                    let emit_calls = self.generate_field_emit_calls(&collected, fields);
                    let constructor = self.generate_dialect_constructor(
                        original_name,
                        Some(name),
                        &collected,
                        fields,
                        true,
                    );
                    quote! {
                        #ast_name::#name(#(#fields),*) => {
                            #emit_calls
                            let dialect_variant: #original_name = #constructor;
                            ctx.context.statement().definition(dialect_variant).new()
                        }
                    }
                } else {
                    let orig_fields = &bindings.original_field_names;
                    let pat: Vec<_> = orig_fields
                        .iter()
                        .zip(fields)
                        .map(|(f, b)| quote! { #f: #b })
                        .collect();
                    let emit_calls = self.generate_field_emit_calls(&collected, fields);
                    let constructor = self.generate_dialect_constructor(
                        original_name,
                        Some(name),
                        &collected,
                        fields,
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

        quote! {
            match self {
                #(#arms)*
            }
        }
    }

    fn generate_field_emit_calls(
        &self,
        collected: &[CollectedField],
        field_vars: &[syn::Ident],
    ) -> TokenStream {
        let crate_path = &self.crate_path;

        let emit_stmts: Vec<_> = collected
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
                            let #emitted_var: ::kirin_ir::SSAValue = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::ResultValue => {
                        quote! {
                            let #emitted_var: ::kirin_ir::ResultValue = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::Block => {
                        quote! {
                            let #emitted_var: ::kirin_ir::Block = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::Successor => {
                        quote! {
                            let #emitted_var: ::kirin_ir::Successor = #crate_path::EmitIR::emit(#var, ctx);
                        }
                    }
                    FieldKind::Region => {
                        quote! {
                            let #emitted_var: ::kirin_ir::Region = #crate_path::EmitIR::emit(#var, ctx);
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

    fn generate_dialect_constructor(
        &self,
        original_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
        collected: &[CollectedField],
        field_vars: &[syn::Ident],
        is_tuple: bool,
    ) -> TokenStream {
        // Generate the field values for the constructor
        // We use the same variable names as in generate_field_emit_calls
        let field_values: Vec<_> = collected
            .iter()
            .zip(field_vars.iter())
            .map(|(field, var)| {
                let emitted_var = syn::Ident::new(
                    &format!("{}_ir", var),
                    proc_macro2::Span::call_site(),
                );

                // For most fields, we convert the emitted IR value to the expected type
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
            })
            .collect();

        if is_tuple {
            match variant_name {
                Some(v) => quote! { #original_name::#v(#(#field_values),*) },
                None => quote! { #original_name(#(#field_values),*) },
            }
        } else {
            let field_assigns: Vec<_> = collected
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
