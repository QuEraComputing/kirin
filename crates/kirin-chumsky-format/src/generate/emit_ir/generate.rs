//! Code generation for the `EmitIR` derive macro.

use kirin_derive_core::ir::{
    VariantRef,
    fields::{FieldCategory, FieldInfo},
};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::field_kind::collect_fields;

use crate::generate::{
    BoundsBuilder, GeneratorConfig, collect_all_value_types_needing_bounds, filter_ast_fields,
    get_fields_in_format,
};

/// Generator for the `EmitIR` trait implementation.
pub struct GenerateEmitIR {
    pub(super) config: GeneratorConfig,
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
        let ast_generics = crate::generate::build_ast_generics(&ir_input.generics, true);
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
    pub(super) fn build_ast_ty_generics(
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

    pub(super) fn language_output_emit_bound(
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

    pub(super) fn ast_needs_language_output_emit_bound(
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

    pub(super) fn statement_needs_language_output_emit_bound(
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

    pub(super) fn statement_contains_statement_recursion_fields(
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

    pub(super) fn ast_fields_contain_statement_recursion_fields(
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

    pub(super) fn is_ir_type_a_type_param(&self, ir_type: &syn::Path, generics: &syn::Generics) -> bool {
        // Type parameter must be a single segment path (e.g., `T`, not `foo::T`)
        if ir_type.segments.len() != 1 {
            return false;
        }

        let ir_type_name = &ir_type.segments[0].ident;

        // Check if this matches any of the struct's type parameters
        generics.type_params().any(|tp| &tp.ident == ir_type_name)
    }

    pub(super) fn generate_emit_impl(
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
        let bounds = BoundsBuilder::new(crate_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.emit_ir_bounds(&value_types);

        // For wrapper enum variants, we need:
        // - Language: From<WrappedDialect> for each wrapped type (so inner.emit(ctx) works)
        // - The wrapped type's AST (with TypeOutput and LanguageOutput) must implement EmitIR
        let wrapper_types = crate::generate::collect_wrapper_types(ir_input);
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
            #[automatically_derived]
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
}
