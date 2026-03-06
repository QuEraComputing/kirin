use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::format::Format;
use crate::validation::validate_format;

use crate::codegen::{GeneratorConfig, format_for_statement};

use super::chain;

/// Generator for the `HasDialectParser` trait implementation.
pub struct GenerateHasDialectParser {
    pub(super) config: GeneratorConfig,
}

impl GenerateHasDialectParser {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>) -> Self {
        Self {
            config: GeneratorConfig::new(ir_input),
        }
    }

    /// Generates the `HasDialectParser` implementation.
    pub fn generate(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let crate_path = &self.config.crate_path;

        let dialect_parser_impl =
            self.generate_dialect_parser_impl(ir_input, &ast_name, crate_path);
        let has_parser_impl = self.generate_has_parser_impl(ir_input, &ast_name, crate_path);

        quote! {
            #dialect_parser_impl
            #has_parser_impl
        }
    }

    /// Generates the parser body for the dialect type.
    pub(super) fn generate_dialect_parser_body(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Struct(s) => {
                self.generate_struct_parser_body(ir_input, &s.0, ast_name, crate_path)
            }
            kirin_derive_toolkit::ir::Data::Enum(e) => {
                self.generate_enum_parser_body(ir_input, e, ast_name, crate_path)
            }
        }
    }

    fn generate_struct_parser_body(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        match self.build_statement_parser(ir_input, stmt, ast_name, None, crate_path) {
            Ok(body) => body,
            Err(err) => err.to_compile_error(),
        }
    }

    fn generate_enum_parser_body(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        data: &kirin_derive_toolkit::ir::DataEnum<ChumskyLayout>,
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
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        variant: Option<&syn::Ident>,
        crate_path: &syn::Path,
    ) -> syn::Result<TokenStream> {
        if let Some(wrapper) = &stmt.wraps {
            return self
                .build_wrapper_parser(ir_input, stmt, ast_name, variant, wrapper, crate_path);
        }

        let format_str = format_for_statement(ir_input, stmt)
            .ok_or_else(|| syn::Error::new(stmt.name.span(), "missing chumsky format attribute"))?;

        let format = Format::parse(&format_str, None)?;
        let collected = stmt.collect_fields();

        let validation_result = validate_format(stmt, &format, &collected)?;
        let occurrences = validation_result.occurrences;

        let ir_type = &ir_input.attrs.ir_type;

        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        let parser_expr = self.build_parser_chain(
            &format,
            &occurrences,
            crate_path,
            ast_name,
            ir_type,
            &type_params,
        )?;

        let var_names: Vec<_> = occurrences.iter().map(|o| o.var_name.clone()).collect();
        let pattern = chain::build_pattern(&var_names);
        let constructor = self.ast_constructor(
            ast_name,
            variant,
            &collected,
            &occurrences,
            crate_path,
            &type_params,
        );

        let type_output = quote! { __TypeOutput };
        let language_output = quote! { __LanguageOutput };
        let return_type =
            self.build_ast_type_reference(ir_input, ast_name, &type_output, &language_output);
        Ok(quote! {{
            use #crate_path::Token;
            #parser_expr.map(|#pattern| -> #return_type { #constructor })
        }})
    }

    fn build_wrapper_parser(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        variant: Option<&syn::Ident>,
        wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> syn::Result<TokenStream> {
        let wrapped_ty = &wrapper.ty;
        let namespace_prefix = format_for_statement(ir_input, stmt);

        let constructor = match variant {
            Some(v) => quote! { #ast_name::#v },
            None => quote! { #ast_name },
        };

        let type_output = quote! { __TypeOutput };
        let language_output = quote! { __LanguageOutput };
        let return_type =
            self.build_ast_type_reference(ir_input, ast_name, &type_output, &language_output);

        let namespace_expr = if let Some(ns) = namespace_prefix {
            quote! {
                {
                    let mut __ns: ::std::vec::Vec<&'static str> = namespace.to_vec();
                    __ns.push(#ns);
                    <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::namespaced_parser::<I, __TypeOutput, __LanguageOutput>(language.clone(), &__ns)
                        .map(|inner| -> #return_type { #constructor(inner) })
                }
            }
        } else {
            quote! {
                <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::namespaced_parser::<I, __TypeOutput, __LanguageOutput>(language.clone(), namespace)
                    .map(|inner| -> #return_type { #constructor(inner) })
            }
        };

        Ok(namespace_expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::parse_derive_input;
    use kirin_test_utils::rustfmt;

    /// Helper: parse DeriveInput, run parser codegen, rustfmt the output.
    fn generate_parser_code(input: syn::DeriveInput) -> String {
        let ir_input = parse_derive_input(&input).expect("Failed to parse derive input");
        let generator = GenerateHasDialectParser::new(&ir_input);
        let tokens = generator.generate(&ir_input);
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_keyword_struct_parser() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[chumsky(crate = kirin_chumsky, format = "{.ret} {value}")]
            struct Return {
                value: Value,
            }
        };
        insta::assert_snapshot!(generate_parser_code(input));
    }

    #[test]
    fn test_keyword_enum_parser() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum ArithOps {
                #[chumsky(format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]
                Add {
                    result: SSAValue,
                    lhs: Value,
                    rhs: Value,
                },
                #[chumsky(format = "{result:name} = {.sub} {lhs}, {rhs} -> {result:type}")]
                Sub {
                    result: SSAValue,
                    lhs: Value,
                    rhs: Value,
                },
            }
        };
        insta::assert_snapshot!(generate_parser_code(input));
    }

    #[test]
    fn test_wrapper_namespace_parser() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum MyLanguage {
                #[wraps]
                #[chumsky(format = "arith")]
                Arith(ArithOps),
                #[wraps]
                Cf(CfOps),
            }
        };
        insta::assert_snapshot!(generate_parser_code(input));
    }
}
