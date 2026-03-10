use kirin_derive_toolkit::ir::{
    VariantRef,
    fields::{FieldCategory, FieldInfo},
};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::codegen::{
    BoundsBuilder, GeneratorConfig, collect_all_value_types_needing_bounds, filter_ast_fields,
    get_fields_in_format,
};

/// Generator for the `EmitIR` trait implementation.
pub struct GenerateEmitIR {
    pub(in crate::codegen) config: GeneratorConfig,
}

impl GenerateEmitIR {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>) -> Self {
        Self {
            config: GeneratorConfig::new(ir_input),
        }
    }

    /// Generates the `EmitIR` implementation.
    pub fn generate(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        if let kirin_derive_toolkit::ir::Data::Struct(data) = &ir_input.data
            && data.0.wraps.is_some()
        {
            return TokenStream::new();
        }

        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_self_name =
            syn::Ident::new(&format!("{}ASTSelf", ir_input.name), ir_input.name.span());
        let ast_generics = crate::codegen::build_ast_generics(&ir_input.generics, true);
        let crate_path = &self.config.crate_path;

        let emit_impl = self.generate_emit_impl(ir_input, &ast_name, &ast_generics, crate_path);
        let ast_self_emit_impl =
            self.generate_ast_self_emit_impl(ir_input, &ast_name, &ast_self_name, crate_path);

        quote! {
            #emit_impl
            #ast_self_emit_impl
        }
    }

    pub(in crate::codegen) fn build_ast_ty_generics(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        if type_params.is_empty() {
            quote! { <'t, TypeOutput, LanguageOutput> }
        } else {
            quote! { <'t, #(#type_params,)* TypeOutput, LanguageOutput> }
        }
    }

    pub(in crate::codegen) fn ast_needs_language_output_emit_bound(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> bool {
        match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Struct(data) => {
                self.statement_needs_language_output_emit_bound(ir_input, &data.0)
            }
            kirin_derive_toolkit::ir::Data::Enum(data) => {
                data.iter_variants().any(|variant| match variant {
                    VariantRef::Wrapper { .. } => false,
                    VariantRef::Regular { stmt, .. } => {
                        self.statement_needs_language_output_emit_bound(ir_input, stmt)
                    }
                })
            }
        }
    }

    pub(in crate::codegen) fn statement_needs_language_output_emit_bound(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
    ) -> bool {
        if stmt.wraps.is_some() {
            return false;
        }

        if !self.statement_contains_statement_recursion_fields(stmt) {
            return false;
        }

        let collected = stmt.collect_fields();
        let fields_in_fmt = get_fields_in_format(ir_input, stmt);
        let ast_fields = filter_ast_fields(&collected, &fields_in_fmt);
        self.ast_fields_contain_statement_recursion_fields(&ast_fields)
    }

    pub(in crate::codegen) fn statement_contains_statement_recursion_fields(
        &self,
        stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
    ) -> bool {
        stmt.iter_all_fields().any(|field| {
            matches!(
                field.category(),
                FieldCategory::Block | FieldCategory::Region
            )
        })
    }

    pub(in crate::codegen) fn ast_fields_contain_statement_recursion_fields(
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

    /// Returns `true` when the generated `EmitIR` impl needs a
    /// `<Language as Dialect>::Type: Placeholder` bound.
    ///
    /// This is required whenever there is at least one `ResultValue` field,
    /// because the `ResultValue` AST type's own `EmitIR` impl requires
    /// `Placeholder` (it may call `placeholder()` when no type annotation
    /// was parsed).
    pub(super) fn needs_placeholder_bound(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> bool {
        use kirin_derive_toolkit::ir::fields::FieldCategory;
        match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Struct(data) => data
                .0
                .fields
                .iter()
                .any(|f| f.category() == FieldCategory::Result),
            kirin_derive_toolkit::ir::Data::Enum(data) => data.variants.iter().any(|stmt| {
                stmt.fields
                    .iter()
                    .any(|f| f.category() == FieldCategory::Result)
            }),
        }
    }

    pub(in crate::codegen) fn is_ir_type_a_type_param(
        &self,
        ir_type: &syn::Path,
        generics: &syn::Generics,
    ) -> bool {
        if ir_type.segments.len() != 1 {
            return false;
        }

        let ir_type_name = &ir_type.segments[0].ident;
        generics.type_params().any(|tp| &tp.ident == ir_type_name)
    }

    pub(in crate::codegen) fn generate_emit_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let helper_ast_generics = crate::codegen::build_ast_generics(&ir_input.generics, false);
        let (helper_impl_generics, _, _) = helper_ast_generics.split_for_impl();
        let (emit_ir_impl_generics, _, _) = ast_generics.split_for_impl();

        let ty_generics = self.build_ast_ty_generics(ir_input);

        let (_, original_ty_generics, _) = ir_input.generics.split_for_impl();

        let emit_body = match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Struct(s) => self.generate_struct_emit(
                ir_input,
                &s.0,
                original_name,
                &original_ty_generics,
                None,
            ),
            kirin_derive_toolkit::ir::Data::Enum(e) => {
                self.generate_enum_emit(ir_input, e, original_name, &original_ty_generics, ast_name)
            }
        };

        let ir_type = &self.config.ir_type;
        let ir_path = &self.config.ir_path;

        let bounds = BoundsBuilder::new(crate_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let helper_impl_ir_type_bound = bounds.ir_type_has_parser_bound(&self.config.ir_type);
        let helper_impl_value_type_bounds = bounds.has_parser_bounds(&value_types);
        let emit_ir_value_type_bounds = bounds.emit_ir_bounds(&value_types);
        let helper_value_type_bounds: Vec<syn::WherePredicate> = value_types
            .iter()
            .flat_map(|ty| {
                [
                    syn::parse_quote! {
                        #ty: #crate_path::HasParser<'t> + 't
                    },
                    syn::parse_quote! {
                        <#ty as #crate_path::HasParser<'t>>::Output:
                            #crate_path::EmitIR<__Language, Output = #ty>
                    },
                ]
            })
            .collect();

        let wrapper_types = crate::codegen::collect_wrapper_types(ir_input);
        let needs_language_output_emit =
            self.ast_needs_language_output_emit_bound(ir_input) || !wrapper_types.is_empty();
        let helper_impl_wrapper_bounds = bounds.has_dialect_parser_bounds(&wrapper_types);
        let emit_ir_wrapper_emit_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| {
                syn::parse_quote! {
                    #ty: #crate_path::HasDialectEmitIR<'t, Language, LanguageOutput>
                }
            })
            .collect();
        let helper_wrapper_emit_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| {
                syn::parse_quote! {
                    #ty: #crate_path::HasDialectEmitIR<'t, __Language, LanguageOutput>
                }
            })
            .collect();

        let ir_type_is_param = self.is_ir_type_a_type_param(ir_type, &ir_input.generics);
        let emit_ir_dialect_type_bound = if ir_type_is_param {
            quote! { Language: #ir_path::Dialect<Type = #ir_type>, }
        } else {
            quote! {}
        };
        let helper_dialect_type_bound = if ir_type_is_param {
            quote! { __Language: #ir_path::Dialect<Type = #ir_type>, }
        } else {
            quote! {}
        };
        let language_output_emit_bound = if needs_language_output_emit {
            quote! {
                LanguageOutput: #crate_path::EmitIR<Language, Output = #ir_path::Statement>,
            }
        } else {
            TokenStream::new()
        };
        let emit_language_output_bound = quote! {
            __EmitLanguageOutput: for<'ctx> Fn(
                &LanguageOutput,
                &mut #crate_path::EmitContext<'ctx, __Language>,
            ) -> ::core::result::Result<#ir_path::Statement, #crate_path::EmitError>,
        };
        let emit_ir_placeholder_bound = if self.needs_placeholder_bound(ir_input) {
            quote! { <Language as #ir_path::Dialect>::Type: #ir_path::Placeholder, }
        } else {
            quote! {}
        };
        let helper_placeholder_bound = if self.needs_placeholder_bound(ir_input) {
            quote! { <__Language as #ir_path::Dialect>::Type: #ir_path::Placeholder, }
        } else {
            quote! {}
        };
        let helper_base_bounds = quote! {
            __Language: #ir_path::Dialect,
            #helper_dialect_type_bound
            #helper_placeholder_bound
            TypeOutput: Clone + PartialEq,
            LanguageOutput: Clone + PartialEq + 't,
            #emit_language_output_bound
            #ir_type: #crate_path::HasParser<'t> + 't,
            <#ir_type as #crate_path::HasParser<'t>>::Output:
                #crate_path::EmitIR<__Language, Output = <__Language as #ir_path::Dialect>::Type>,
        };
        let emit_ir_base_bounds = quote! {
            Language: #ir_path::Dialect + From<#original_name #original_ty_generics>,
            #emit_ir_dialect_type_bound
            #emit_ir_placeholder_bound
            TypeOutput: Clone + PartialEq,
            LanguageOutput: Clone + PartialEq + 't,
            #language_output_emit_bound
            #ir_type: #crate_path::HasParser<'t> + 't,
            <#ir_type as #crate_path::HasParser<'t>>::Output:
                #crate_path::EmitIR<Language, Output = <Language as #ir_path::Dialect>::Type>,
        };
        let helper_impl_base_bounds = quote! {
            TypeOutput: Clone + PartialEq + 't,
            LanguageOutput: Clone + PartialEq + 't,
        };

        let mut helper_impl_bounds = vec![
            helper_impl_base_bounds,
            quote! { #helper_impl_ir_type_bound, },
        ];
        if !helper_impl_value_type_bounds.is_empty() {
            let bounds_tokens = helper_impl_value_type_bounds.iter().map(|b| quote! { #b, });
            helper_impl_bounds.push(quote! { #(#bounds_tokens)* });
        }
        if !helper_impl_wrapper_bounds.is_empty() {
            let bounds_tokens = helper_impl_wrapper_bounds.iter().map(|b| quote! { #b, });
            helper_impl_bounds.push(quote! { #(#bounds_tokens)* });
        }

        let mut helper_bounds = vec![helper_base_bounds];
        if !helper_value_type_bounds.is_empty() {
            let bounds_tokens = helper_value_type_bounds.iter().map(|b| quote! { #b, });
            helper_bounds.push(quote! { #(#bounds_tokens)* });
        }
        if !helper_wrapper_emit_bounds.is_empty() {
            let emit_tokens = helper_wrapper_emit_bounds.iter().map(|b| quote! { #b, });
            helper_bounds.push(quote! { #(#emit_tokens)* });
        }

        let mut emit_ir_bounds = vec![emit_ir_base_bounds];
        if !emit_ir_value_type_bounds.is_empty() {
            let bounds_tokens = emit_ir_value_type_bounds.iter().map(|b| quote! { #b, });
            emit_ir_bounds.push(quote! { #(#bounds_tokens)* });
        }
        if !emit_ir_wrapper_emit_bounds.is_empty() {
            let emit_tokens = emit_ir_wrapper_emit_bounds.iter().map(|b| quote! { #b, });
            emit_ir_bounds.push(quote! { #(#emit_tokens)* });
        }

        let helper_impl_where_clause = quote! { where #(#helper_impl_bounds)* };
        let helper_where_clause = quote! { where #(#helper_bounds)* };
        let emit_ir_where_clause = quote! { where #(#emit_ir_bounds)* };
        let emit_language_output = if needs_language_output_emit {
            quote! { &|stmt, ctx| #crate_path::EmitIR::emit(stmt, ctx) }
        } else {
            quote! { &|_, _| unreachable!() }
        };

        quote! {
            impl #helper_impl_generics #ast_name #ty_generics
            #helper_impl_where_clause
            {
                fn emit_with<__Language, __EmitLanguageOutput>(
                    &self,
                    ctx: &mut #crate_path::EmitContext<'_, __Language>,
                    emit_language_output: &__EmitLanguageOutput,
                ) -> ::core::result::Result<#original_name #original_ty_generics, #crate_path::EmitError>
                #helper_where_clause
                {
                    #emit_body
                }
            }

            #[automatically_derived]
            impl #emit_ir_impl_generics #crate_path::EmitIR<Language> for #ast_name #ty_generics
            #emit_ir_where_clause
            {
                type Output = #ir_path::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> ::core::result::Result<Self::Output, #crate_path::EmitError> {
                    let dialect_variant = self.emit_with(ctx, #emit_language_output)?;
                    Ok(ctx.stage.statement().definition(dialect_variant).new())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::parse_derive_input;
    use kirin_test_utils::rustfmt;

    fn generate_emit_ir_code(input: syn::DeriveInput) -> String {
        let ir_input = parse_derive_input(&input).expect("Failed to parse derive input");
        let generator = GenerateEmitIR::new(&ir_input);
        let tokens = generator.generate(&ir_input);
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_emit_ir_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[chumsky(crate = kirin_chumsky, format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]
            struct Add {
                result: SSAValue,
                lhs: Value,
                rhs: Value,
            }
        };
        insta::assert_snapshot!(generate_emit_ir_code(input));
    }

    #[test]
    fn test_emit_ir_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum ArithOps {
                #[chumsky(format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]
                Add {
                    result: SSAValue,
                    lhs: Value,
                    rhs: Value,
                },
                #[chumsky(format = "{result:name} = {.neg} {operand} -> {result:type}")]
                Neg {
                    result: SSAValue,
                    operand: Value,
                },
            }
        };
        insta::assert_snapshot!(generate_emit_ir_code(input));
    }
}
