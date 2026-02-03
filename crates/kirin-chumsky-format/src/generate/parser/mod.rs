//! Code generation for the `HasDialectParser` derive macro.

mod chain;
mod impl_gen;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::collect_fields;
use crate::format::Format;
use crate::validation::validate_format;

use super::{GeneratorConfig, format_for_statement};

/// Generator for the `HasDialectParser` trait implementation.
pub struct GenerateHasDialectParser {
    pub(super) config: GeneratorConfig,
}

impl GenerateHasDialectParser {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        Self {
            config: GeneratorConfig::new(ir_input),
        }
    }

    /// Generates the `HasDialectParser` implementation.
    ///
    /// Only the dialect type implements `HasDialectParser`, with the AST type as Output.
    /// The AST type itself does not implement `HasDialectParser`.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let crate_path = &self.config.crate_path;

        // Generate HasDialectParser impl for the original type (dialect)
        let dialect_parser_impl =
            self.generate_dialect_parser_impl(ir_input, &ast_name, crate_path);

        // Generate HasParser impl for the original type
        let has_parser_impl = self.generate_has_parser_impl(ir_input, &ast_name, crate_path);

        quote! {
            #dialect_parser_impl
            #has_parser_impl
        }
    }

    /// Generates the parser body for the dialect type.
    fn generate_dialect_parser_body(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(s) => {
                self.generate_struct_parser_body(ir_input, &s.0, ast_name, crate_path)
            }
            kirin_derive_core::ir::Data::Enum(e) => {
                self.generate_enum_parser_body(ir_input, e, ast_name, crate_path)
            }
        }
    }

    /// Generates the struct parser body (without the impl wrapper).
    fn generate_struct_parser_body(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        match self.build_statement_parser(ir_input, stmt, ast_name, None, crate_path) {
            Ok(body) => body,
            Err(err) => err.to_compile_error(),
        }
    }

    /// Generates the enum parser body (without the impl wrapper).
    fn generate_enum_parser_body(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let mut variant_parsers = Vec::new();
        for variant in &data.variants {
            let parser = self.build_statement_parser(
                ir_input,
                variant,
                ast_name,
                Some(&variant.name),
                crate_path,
            );
            match parser {
                Ok(p) => variant_parsers.push(p),
                Err(err) => variant_parsers.push(err.to_compile_error()),
            }
        }

        if variant_parsers.is_empty() {
            quote! { #crate_path::chumsky::prelude::empty().map(|_: ()| unreachable!()) }
        } else {
            variant_parsers
                .into_iter()
                .reduce(|acc, parser| quote! { #acc.or(#parser) })
                .unwrap()
        }
    }

    fn build_statement_parser(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        variant: Option<&syn::Ident>,
        crate_path: &syn::Path,
    ) -> syn::Result<TokenStream> {
        // Check if this is a wrapper variant
        if let Some(wrapper) = &stmt.wraps {
            return self.build_wrapper_parser(ir_input, ast_name, variant, wrapper, crate_path);
        }

        let format_str = format_for_statement(ir_input, stmt)
            .ok_or_else(|| syn::Error::new(stmt.name.span(), "missing chumsky format attribute"))?;

        let format = Format::parse(&format_str, None)?;
        let collected = collect_fields(stmt);

        // Use the ValidationVisitor to validate and get field occurrences
        let validation_result = validate_format(stmt, &format, &collected)?;
        let occurrences = validation_result.occurrences;

        // Get the type lattice for type annotation parsers
        let type_lattice = &ir_input.attrs.type_lattice;

        // Build parser chain properly handling the tuple nesting
        let parser_expr =
            self.build_parser_chain(&format, &occurrences, crate_path, ast_name, type_lattice)?;

        // Generate pattern matching for the parser output
        let var_names: Vec<_> = occurrences.iter().map(|o| o.var_name.clone()).collect();
        let pattern = chain::build_pattern(&var_names);
        let constructor =
            self.ast_constructor(ast_name, variant, &collected, &occurrences, crate_path);

        // Use explicit return type annotation to pin the lifetimes correctly.
        // Without this, Rust would infer anonymous lifetimes '_ for the constructor.
        // Use generic Language since this is inside HasDialectParser::recursive_parser.
        let language = quote! { Language };
        let return_type = self.build_ast_type_reference(ir_input, ast_name, &language);
        Ok(quote! {{
            use #crate_path::Token;
            #parser_expr.map(|#pattern| -> #return_type { #constructor })
        }})
    }

    fn build_wrapper_parser(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        variant: Option<&syn::Ident>,
        wrapper: &kirin_derive_core::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> syn::Result<TokenStream> {
        let wrapped_ty = &wrapper.ty;

        let constructor = match variant {
            Some(v) => quote! { #ast_name::#v },
            None => quote! { #ast_name },
        };

        // Use explicit return type annotation to pin the lifetimes correctly
        // Use generic Language since this is inside HasDialectParser::recursive_parser.
        let language = quote! { Language };
        let return_type = self.build_ast_type_reference(ir_input, ast_name, &language);
        Ok(quote! {
            <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src, Language>>::recursive_parser(language.clone())
                .map(|inner| -> #return_type { #constructor(inner) })
        })
    }
}
