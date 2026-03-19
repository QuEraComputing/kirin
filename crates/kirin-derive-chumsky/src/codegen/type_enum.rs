//! Code generation for type enum derives.
//!
//! When all variants of an enum are unit variants with `#[chumsky(format = "...")]`
//! and no `#[wraps]` or IR fields, the enum is treated as a "type enum" and gets
//! simpler generated code: `Display`, `HasParser`, `DirectlyParsable`, and
//! `PrettyPrintViaDisplay`.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::PrettyPrintLayout;
use crate::codegen::helpers::format_for_statement;

/// Checks whether the given input represents a type enum.
///
/// A type enum is an enum where:
/// - All variants are unit (no fields)
/// - No variant has `#[wraps]`
/// - Every variant has a `#[chumsky(format = "...")]` attribute
pub fn is_type_enum<L>(ir_input: &kirin_derive_toolkit::ir::Input<L>) -> bool
where
    L: kirin_derive_toolkit::ir::Layout<ExtraStatementAttrs = crate::ChumskyStatementAttrs>,
    L::ExtraGlobalAttrs: super::helpers::HasGlobalFormat,
{
    let data = match &ir_input.data {
        kirin_derive_toolkit::ir::Data::Enum(data) => data,
        _ => return false,
    };

    // Must have at least one variant
    if data.variants.is_empty() {
        return false;
    }

    data.variants.iter().all(|v| {
        v.wraps.is_none()
            && v.collect_fields().is_empty()
            && format_for_statement(ir_input, v).is_some()
    })
}

/// Generates `Display`, `HasParser`, and `DirectlyParsable` impls for a type enum.
pub struct GenerateTypeEnum;

impl GenerateTypeEnum {
    /// Generates all impls for the `HasParser` derive on a type enum.
    pub fn generate(ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>) -> TokenStream {
        let display_impl = Self::generate_display(ir_input);
        let has_parser_impl = Self::generate_has_parser(ir_input);
        let directly_parsable_impl = Self::generate_directly_parsable(ir_input);

        quote! {
            #display_impl
            #has_parser_impl
            #directly_parsable_impl
        }
    }

    /// Generates the `PrettyPrintViaDisplay` marker impl for the `PrettyPrint` derive.
    pub fn generate_pretty_print_via_display(
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
    ) -> TokenStream {
        let name = &ir_input.name;
        let (impl_generics, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let prettyless_path: syn::Path = ir_input
            .extra_attrs
            .crate_path
            .as_ref()
            .cloned()
            .unwrap_or_else(|| syn::parse_quote!(::kirin::pretty));

        quote! {
            #[automatically_derived]
            impl #impl_generics #prettyless_path::PrettyPrintViaDisplay
                for #name #ty_generics
            #where_clause
            {}
        }
    }

    fn generate_display(ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>) -> TokenStream {
        let name = &ir_input.name;
        let (impl_generics, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let data = match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Enum(data) => data,
            _ => unreachable!("is_type_enum should have verified this is an enum"),
        };

        let arms: Vec<TokenStream> = data
            .variants
            .iter()
            .map(|v| {
                let variant_name = &v.name;
                let format_str = format_for_statement(ir_input, v)
                    .expect("is_type_enum verified all variants have format strings");
                quote! {
                    #name::#variant_name => f.write_str(#format_str),
                }
            })
            .collect();

        quote! {
            #[automatically_derived]
            impl #impl_generics ::core::fmt::Display for #name #ty_generics
            #where_clause
            {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        #(#arms)*
                    }
                }
            }
        }
    }

    fn generate_has_parser(
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        let name = &ir_input.name;
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let crate_path = ir_input.extra_crate_path(|| syn::parse_quote!(::kirin::parsers));

        let data = match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Enum(data) => data,
            _ => unreachable!("is_type_enum should have verified this is an enum"),
        };

        let select_arms: Vec<TokenStream> = data
            .variants
            .iter()
            .map(|v| {
                let variant_name = &v.name;
                let format_str = format_for_statement(ir_input, v)
                    .expect("is_type_enum verified all variants have format strings");
                quote! {
                    #crate_path::Token::Identifier(#format_str) => #name::#variant_name
                }
            })
            .collect();

        // Build a label from the enum name in kebab-case
        let label = to_kebab_case(&name.to_string());

        // Add 't lifetime to impl generics
        let mut generics_with_lifetime = ir_input.generics.clone();
        let t_lt = syn::Lifetime::new("'t", proc_macro2::Span::call_site());
        if !generics_with_lifetime
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "t"))
        {
            generics_with_lifetime.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(t_lt)),
            );
        }
        let (impl_generics, _, _) = generics_with_lifetime.split_for_impl();

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasParser<'t> for #name #ty_generics
            #where_clause
            {
                type Output = #name #ty_generics;

                fn parser<__I>() -> #crate_path::BoxedParser<'t, __I, Self::Output>
                where
                    __I: #crate_path::TokenInput<'t>,
                {
                    use #crate_path::chumsky::prelude::*;
                    select! {
                        #(#select_arms),*
                    }
                    .labelled(#label)
                    .boxed()
                }
            }
        }
    }

    fn generate_directly_parsable(
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        let name = &ir_input.name;
        let (impl_generics, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        let crate_path = ir_input.extra_crate_path(|| syn::parse_quote!(::kirin::parsers));

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::DirectlyParsable for #name #ty_generics
            #where_clause
            {}
        }
    }
}

/// Converts a PascalCase name to kebab-case.
fn to_kebab_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('-');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{parse_derive_input, parse_pretty_derive_input};
    use kirin_test_utils::rustfmt;

    fn generate_type_enum_code(input: syn::DeriveInput) -> String {
        let ir_input = parse_derive_input(&input).expect("Failed to parse derive input");
        assert!(is_type_enum(&ir_input), "Expected type enum detection");
        let tokens = GenerateTypeEnum::generate(&ir_input);
        rustfmt(tokens.to_string())
    }

    fn generate_type_enum_pretty_print_code(input: syn::DeriveInput) -> String {
        let ir_input = parse_pretty_derive_input(&input).expect("Failed to parse derive input");
        assert!(is_type_enum(&ir_input), "Expected type enum detection");
        let tokens = GenerateTypeEnum::generate_pretty_print_via_display(&ir_input);
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_type_enum_display_and_parser() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[derive(Clone)]
            enum ArithType {
                #[chumsky(format = "i8")]
                I8,
                #[chumsky(format = "i16")]
                I16,
                #[chumsky(format = "i32")]
                I32,
                #[chumsky(format = "i64")]
                I64,
                #[chumsky(format = "f32")]
                F32,
                #[chumsky(format = "f64")]
                F64,
            }
        };
        insta::assert_snapshot!(generate_type_enum_code(input));
    }

    #[test]
    fn test_type_enum_pretty_print_via_display() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[derive(Clone)]
            enum ArithType {
                #[chumsky(format = "i8")]
                I8,
                #[chumsky(format = "i32")]
                I32,
                #[chumsky(format = "f64")]
                F64,
            }
        };
        insta::assert_snapshot!(generate_type_enum_pretty_print_code(input));
    }

    #[test]
    fn test_not_type_enum_with_fields() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[chumsky(format = "$ret {value}")]
            struct Return {
                value: Value,
            }
        };
        let ir_input = parse_derive_input(&input).expect("Failed to parse derive input");
        assert!(!is_type_enum(&ir_input), "Struct should not be a type enum");
    }

    #[test]
    fn test_not_type_enum_with_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum MyLanguage {
                #[wraps]
                #[chumsky(format = "arith")]
                Arith(ArithOps),
            }
        };
        let ir_input = parse_derive_input(&input).expect("Failed to parse derive input");
        assert!(
            !is_type_enum(&ir_input),
            "Enum with #[wraps] should not be a type enum"
        );
    }

    #[test]
    fn test_kebab_case() {
        assert_eq!(to_kebab_case("ArithType"), "arith-type");
        assert_eq!(to_kebab_case("SimpleType"), "simple-type");
        assert_eq!(to_kebab_case("ABC"), "a-b-c");
    }
}
