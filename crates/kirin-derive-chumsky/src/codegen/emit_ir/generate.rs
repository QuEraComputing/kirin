use kirin_derive_toolkit::ir::{
    VariantRef,
    fields::{FieldCategory, FieldInfo},
};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::codegen::{
    GeneratorConfig, ImplBounds, WhereClauseExt, filter_ast_fields, get_fields_in_format,
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
                FieldCategory::Block
                    | FieldCategory::Region
                    | FieldCategory::DiGraph
                    | FieldCategory::UnGraph
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
                FieldCategory::Block
                    | FieldCategory::Region
                    | FieldCategory::DiGraph
                    | FieldCategory::UnGraph
            )
        })
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

        let ir_path = &self.config.ir_path;
        let bounds = ImplBounds::from_input(ir_input, &self.config);
        let lt_t: syn::Lifetime = syn::parse_quote!('t);
        let lang_emit_ir = quote! { __Language };
        let lang_trait_impl = quote! { Language };
        let lang_output = quote! { LanguageOutput };

        let needs_language_output_emit =
            self.ast_needs_language_output_emit_bound(ir_input) || bounds.has_wrappers();

        // --- helper_impl: struct-level `impl FooAST { fn emit_with(...) }` ---
        let mut helper_impl_wc = syn::WhereClause {
            where_token: syn::token::Where::default(),
            predicates: syn::punctuated::Punctuated::new(),
        };
        helper_impl_wc
            .predicates
            .push(syn::parse_quote! { TypeOutput: Clone + PartialEq + 't });
        helper_impl_wc
            .predicates
            .push(syn::parse_quote! { LanguageOutput: Clone + PartialEq + 't });
        helper_impl_wc
            .predicates
            .push(bounds.ir_type_has_parser(&lt_t));
        helper_impl_wc
            .predicates
            .extend(bounds.value_types_has_parser(&lt_t));
        helper_impl_wc
            .predicates
            .extend(bounds.wrappers_has_dialect_parser(&lt_t));

        // --- helper: method-level where clause on `emit_with` ---
        let mut helper_wc = syn::WhereClause {
            where_token: syn::token::Where::default(),
            predicates: syn::punctuated::Punctuated::new(),
        };
        helper_wc
            .predicates
            .push(syn::parse_quote! { __Language: #ir_path::Dialect });
        helper_wc.push_opt(bounds.dialect_type_bound(&lang_emit_ir));
        if bounds.needs_placeholder() {
            helper_wc
                .predicates
                .extend(bounds.placeholder_predicates(&lang_emit_ir));
        }
        helper_wc
            .predicates
            .push(syn::parse_quote! { TypeOutput: Clone + PartialEq });
        helper_wc
            .predicates
            .push(syn::parse_quote! { LanguageOutput: Clone + PartialEq + 't });
        helper_wc.predicates.push(syn::parse_quote! {
            __EmitLanguageOutput: for<'ctx> Fn(
                &LanguageOutput,
                &mut #crate_path::EmitContext<'ctx, __Language>,
            ) -> ::core::result::Result<#ir_path::Statement, #crate_path::EmitError>
        });
        helper_wc.predicates.push(bounds.ir_type_has_parser(&lt_t));
        helper_wc
            .predicates
            .push(bounds.ir_type_emit_ir(&lt_t, &lang_emit_ir));
        helper_wc
            .predicates
            .extend(bounds.value_types_all(&lt_t, &lang_emit_ir));
        helper_wc
            .predicates
            .extend(bounds.wrappers_emit_ir(&lt_t, &lang_emit_ir, &lang_output));

        // --- emit_ir: `impl EmitIR<Language> for FooAST` ---
        let mut emit_ir_wc = syn::WhereClause {
            where_token: syn::token::Where::default(),
            predicates: syn::punctuated::Punctuated::new(),
        };
        emit_ir_wc.predicates.push(syn::parse_quote! {
            Language: #ir_path::Dialect + From<#original_name #original_ty_generics>
        });
        emit_ir_wc.push_opt(bounds.dialect_type_bound(&lang_trait_impl));
        if bounds.needs_placeholder() {
            emit_ir_wc
                .predicates
                .extend(bounds.placeholder_predicates(&lang_trait_impl));
        }
        emit_ir_wc
            .predicates
            .push(syn::parse_quote! { TypeOutput: Clone + PartialEq });
        emit_ir_wc
            .predicates
            .push(syn::parse_quote! { LanguageOutput: Clone + PartialEq + 't });
        if needs_language_output_emit {
            emit_ir_wc.predicates.push(syn::parse_quote! {
                LanguageOutput: #crate_path::EmitIR<Language, Output = #ir_path::Statement>
            });
        }
        emit_ir_wc.predicates.push(bounds.ir_type_has_parser(&lt_t));
        emit_ir_wc
            .predicates
            .push(bounds.ir_type_emit_ir(&lt_t, &lang_trait_impl));
        emit_ir_wc
            .predicates
            .extend(bounds.value_types_all(&lt_t, &lang_trait_impl));
        emit_ir_wc.predicates.extend(bounds.wrappers_emit_ir(
            &lt_t,
            &lang_trait_impl,
            &lang_output,
        ));

        let emit_language_output = if needs_language_output_emit {
            quote! { &|stmt, ctx| #crate_path::EmitIR::emit(stmt, ctx) }
        } else {
            quote! { &|_, _| unreachable!() }
        };

        quote! {
            impl #helper_impl_generics #ast_name #ty_generics
            #helper_impl_wc
            {
                fn emit_with<__Language, __EmitLanguageOutput>(
                    &self,
                    ctx: &mut #crate_path::EmitContext<'_, __Language>,
                    emit_language_output: &__EmitLanguageOutput,
                ) -> ::core::result::Result<#original_name #original_ty_generics, #crate_path::EmitError>
                #helper_wc
                {
                    #emit_body
                }
            }

            #[automatically_derived]
            impl #emit_ir_impl_generics #crate_path::EmitIR<Language> for #ast_name #ty_generics
            #emit_ir_wc
            {
                type Output = #ir_path::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> ::core::result::Result<Self::Output, #crate_path::EmitError> {
                    let dialect_variant = self.emit_with(ctx, #emit_language_output)?;
                    Ok(ctx.stage.statement(dialect_variant))
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
