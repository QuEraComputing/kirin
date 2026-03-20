use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::quote;

use crate::PrettyPrintLayout;
use kirin_derive_toolkit::ir::fields::FieldInfo;
use kirin_lexer::Token;

use crate::field_kind;
use crate::format::{Format, FormatElement};

use crate::codegen::generate_enum_match;

use super::GeneratePrettyPrint;

fn build_field_map(
    collected: &[FieldInfo<PrettyPrintLayout>],
) -> IndexMap<String, (usize, &FieldInfo<PrettyPrintLayout>)> {
    let mut map = IndexMap::new();
    for (idx, field) in collected.iter().enumerate() {
        map.insert(field.index.to_string(), (idx, field));

        if let Some(ident) = &field.ident {
            map.insert(ident.to_string(), (idx, field));
        }
    }
    map
}

fn tokens_to_string_with_spacing(
    tokens: &[Token],
    add_leading_space: bool,
    add_trailing_space: bool,
) -> String {
    let mut result = String::new();

    if add_leading_space && !tokens.is_empty() {
        let needs_leading_space = !matches!(
            tokens.first(),
            Some(Token::Comma) | Some(Token::RBrace) | Some(Token::RParen) | Some(Token::RBracket)
        );
        if needs_leading_space {
            result.push(' ');
        }
    }

    for (i, token) in tokens.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        match token {
            Token::EscapedLBrace => result.push('{'),
            Token::EscapedRBrace => result.push('}'),
            other => result.push_str(&other.to_string()),
        }
    }

    if add_trailing_space && !tokens.is_empty() {
        let needs_trailing_space = !matches!(
            tokens.last(),
            Some(Token::LBrace) | Some(Token::LParen) | Some(Token::LBracket)
        );
        if needs_trailing_space {
            result.push(' ');
        }
    }

    result
}

impl GeneratePrettyPrint {
    pub(super) fn generate_pretty_print(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
    ) -> TokenStream {
        let dialect_name = &ir_input.name;
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();
        let prettyless_path = &self.prettyless_path;
        let ir_path = Self::ir_path(ir_input);

        let print_body = match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Struct(s) => {
                self.generate_struct_print(ir_input, &s.0, dialect_name, None)
            }
            kirin_derive_toolkit::ir::Data::Enum(e) => {
                self.generate_enum_print(ir_input, e, dialect_name)
            }
        };

        let (impl_generics, _, _) = ir_input.generics.split_for_impl();

        // Generate prints_result_names override for new-format dialects
        let prints_result_names = self.generate_prints_result_names(ir_input);

        quote! {
            #[automatically_derived]
            impl #impl_generics #prettyless_path::PrettyPrint
                for #dialect_name #ty_generics
            #where_clause
            {
                fn namespaced_pretty_print<'a, __L: #ir_path::Dialect + #prettyless_path::PrettyPrint>(
                    &self,
                    doc: &'a #prettyless_path::Document<'a, __L>,
                    namespace: &[&str],
                ) -> #prettyless_path::ArenaDoc<'a>
                where
                    __L::Type: ::core::fmt::Display,
                {
                    use #prettyless_path::DocAllocator;
                    #print_body
                }

                #prints_result_names
            }
        }
    }

    pub(super) fn generate_wrapper_struct_pretty_print(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
        wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
    ) -> TokenStream {
        let dialect_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;
        let prettyless_path = &self.prettyless_path;
        let ir_path = Self::ir_path(ir_input);

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();
        let (impl_generics, _, _) = ir_input.generics.split_for_impl();

        let wrapped_bound: syn::WherePredicate =
            syn::parse_quote! { #wrapped_ty: #prettyless_path::PrettyPrint };

        let final_where = match where_clause {
            Some(wc) => {
                let mut combined = wc.clone();
                combined.predicates.push(wrapped_bound);
                quote! { #combined }
            }
            None => {
                quote! { where #wrapped_bound }
            }
        };

        quote! {
            #[automatically_derived]
            impl #impl_generics #prettyless_path::PrettyPrint
                for #dialect_name #ty_generics
            #final_where
            {
                fn namespaced_pretty_print<'a, __L: #ir_path::Dialect + #prettyless_path::PrettyPrint>(
                    &self,
                    doc: &'a #prettyless_path::Document<'a, __L>,
                    namespace: &[&str],
                ) -> #prettyless_path::ArenaDoc<'a>
                where
                    __L::Type: ::core::fmt::Display,
                {
                    let inner = &self.0;
                    inner.namespaced_pretty_print(doc, namespace)
                }
            }
        }
    }

    fn generate_struct_print(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<PrettyPrintLayout>,
        dialect_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
    ) -> TokenStream {
        let (pattern, print_expr) =
            self.build_print_components(ir_input, stmt, dialect_name, variant_name);

        quote! {
            let #pattern = self;
            #print_expr
        }
    }

    fn build_print_components(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<PrettyPrintLayout>,
        dialect_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
    ) -> (TokenStream, TokenStream) {
        let format_str = crate::codegen::format_for_statement(ir_input, stmt)
            .expect("Statement must have format string");

        let format = Format::parse(&format_str, None).expect("Format string should be valid");

        let collected = stmt.collect_fields();
        let field_map = build_field_map(&collected);
        let bindings = stmt.field_bindings("f");
        let fields = &bindings.field_idents;

        let ir_path = Self::ir_path(ir_input);
        let print_expr = self.generate_format_print(&format, &field_map, &collected, fields, &ir_path);

        let pattern = if bindings.is_empty() {
            match variant_name {
                Some(v) if bindings.is_tuple => quote! { #dialect_name::#v },
                Some(v) => quote! { #dialect_name::#v {} },
                None if bindings.is_tuple => quote! { #dialect_name },
                None => quote! { #dialect_name {} },
            }
        } else if bindings.is_tuple {
            match variant_name {
                Some(v) => quote! { #dialect_name::#v(#(#fields),*) },
                None => quote! { #dialect_name(#(#fields),*) },
            }
        } else {
            let orig_fields = &bindings.original_field_names;
            let pat: Vec<_> = orig_fields
                .iter()
                .zip(fields)
                .map(|(f, b)| quote! { #f: #b })
                .collect();
            match variant_name {
                Some(v) => quote! { #dialect_name::#v { #(#pat),* } },
                None => quote! { #dialect_name { #(#pat),* } },
            }
        };

        (pattern, print_expr)
    }

    fn generate_enum_print(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
        data: &kirin_derive_toolkit::ir::DataEnum<PrettyPrintLayout>,
        dialect_name: &syn::Ident,
    ) -> TokenStream {
        generate_enum_match(
            dialect_name,
            data,
            |_name, _wrapper, stmt| {
                let namespace_prefix = crate::codegen::format_for_statement(ir_input, stmt);
                if let Some(ns) = namespace_prefix {
                    quote! {
                        {
                            let mut __ns: ::std::vec::Vec<&str> = namespace.to_vec();
                            __ns.push(#ns);
                            inner.namespaced_pretty_print(doc, &__ns)
                        }
                    }
                } else {
                    quote! {
                        inner.namespaced_pretty_print(doc, namespace)
                    }
                }
            },
            |name, variant| self.generate_variant_print(ir_input, variant, dialect_name, name),
            None,
        )
    }

    fn generate_variant_print(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
        variant: &kirin_derive_toolkit::ir::Statement<PrettyPrintLayout>,
        dialect_name: &syn::Ident,
        variant_name: &syn::Ident,
    ) -> TokenStream {
        let (pattern, print_expr) =
            self.build_print_components(ir_input, variant, dialect_name, Some(variant_name));

        quote! {
            #pattern => {
                #print_expr
            }
        }
    }

    pub(super) fn generate_format_print(
        &self,
        format: &Format<'_>,
        field_map: &IndexMap<String, (usize, &FieldInfo<PrettyPrintLayout>)>,
        _collected: &[FieldInfo<PrettyPrintLayout>],
        field_vars: &[syn::Ident],
        ir_path: &syn::Path,
    ) -> TokenStream {
        let prettyless_path = &self.prettyless_path;
        let elements = format.elements();

        let mut parts: Vec<TokenStream> = Vec::new();

        for (i, elem) in elements.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == elements.len() - 1;
            let prev_is_field_like = i > 0
                && matches!(
                    elements[i - 1],
                    FormatElement::Field(_, _) | FormatElement::Keyword(_) | FormatElement::Context(_)
                );
            let next_is_field_like = !is_last
                && matches!(
                    elements[i + 1],
                    FormatElement::Field(_, _) | FormatElement::Keyword(_) | FormatElement::Context(_)
                );

            match elem {
                FormatElement::Token(tokens) => {
                    let text = tokens_to_string_with_spacing(
                        tokens,
                        prev_is_field_like,
                        next_is_field_like,
                    );
                    parts.push(quote! { doc.text(#text) });
                }
                FormatElement::Keyword(name) => {
                    let keyword_expr = quote! {
                        (if namespace.is_empty() {
                            doc.text(#name)
                        } else {
                            let mut __s = namespace.join(".");
                            __s.push('.');
                            __s.push_str(#name);
                            doc.text(__s)
                        })
                    };

                    // Add spacing like fields do
                    if !is_first && prev_is_field_like {
                        parts.push(quote! { doc.text(" ") });
                    }

                    parts.push(keyword_expr);
                }
                FormatElement::Field(name, opt) => {
                    let name_str = name.to_string();
                    if let Some((idx, field)) = field_map.get(&name_str) {
                        let var = &field_vars[*idx];
                        let var_ref = quote! { #var };

                        let print_expr =
                            field_kind::print_expr(field, prettyless_path, &var_ref, opt, Some(ir_path));

                        if !is_first && prev_is_field_like {
                            parts.push(quote! { doc.text(" ") });
                        }

                        parts.push(print_expr);
                    }
                }
                FormatElement::Context(_) => {
                    // Context projection ({:name}) prints the enclosing function name
                    if !is_first && prev_is_field_like {
                        parts.push(quote! { doc.text(" ") });
                    }
                    parts.push(quote! { doc.print_function_name() });
                }
            }
        }

        if parts.is_empty() {
            quote! { doc.nil() }
        } else {
            let first = &parts[0];
            let rest = &parts[1..];
            quote! {
                #first #(+ #rest)*
            }
        }
    }

    /// Generates the `prints_result_names` method override for the PrettyPrint impl.
    ///
    /// All dialects now use new-format mode where result names are printed by the
    /// statement-level printer. For wrapper types, we delegate to the inner type.
    fn generate_prints_result_names(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
    ) -> TokenStream {
        use kirin_derive_toolkit::ir::VariantRef;
        let prettyless_path = &self.prettyless_path;

        match &ir_input.data {
            kirin_derive_toolkit::ir::Data::Struct(s) => {
                if s.0.wraps.is_some() {
                    // Wrapper struct: delegate to inner type
                    quote! {
                        fn prints_result_names(&self) -> bool {
                            #prettyless_path::PrettyPrint::prints_result_names(&self.0)
                        }
                    }
                } else {
                    // All non-wrapper structs return false (new-format)
                    quote! { fn prints_result_names(&self) -> bool { false } }
                }
            }
            kirin_derive_toolkit::ir::Data::Enum(e) => {
                let has_wrappers = e
                    .iter_variants()
                    .any(|v| matches!(v, VariantRef::Wrapper { .. }));

                if !has_wrappers {
                    // All regular variants use new-format
                    quote! { fn prints_result_names(&self) -> bool { false } }
                } else {
                    // Generate per-variant dispatch for wrapper variants
                    let dialect_name = &ir_input.name;
                    let arms: Vec<_> = e
                        .iter_variants()
                        .map(|variant| match variant {
                            VariantRef::Wrapper { name, .. } => {
                                quote! {
                                    #dialect_name::#name(inner) => {
                                        #prettyless_path::PrettyPrint::prints_result_names(inner)
                                    }
                                }
                            }
                            VariantRef::Regular { name, .. } => {
                                quote! {
                                    #dialect_name::#name { .. } => { false }
                                }
                            }
                        })
                        .collect();

                    let wildcard = if e.has_hidden_variants {
                        quote! { _ => false }
                    } else {
                        quote! {}
                    };

                    quote! {
                        fn prints_result_names(&self) -> bool {
                            match self {
                                #(#arms)*
                                #wildcard
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::parse_pretty_derive_input;
    use kirin_test_utils::rustfmt;

    /// Helper: parse DeriveInput, run pretty-print codegen, rustfmt the output.
    fn generate_pretty_print_code(input: syn::DeriveInput) -> String {
        let ir_input = parse_pretty_derive_input(&input).expect("Failed to parse derive input");
        let generator = GeneratePrettyPrint::new(&ir_input);
        let tokens = generator.generate(&ir_input);
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_keyword_struct_pretty_print() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[chumsky(format = "$ret {value}")]
            struct Return {
                value: Value,
            }
        };
        insta::assert_snapshot!(generate_pretty_print_code(input));
    }

    #[test]
    fn test_keyword_enum_pretty_print() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum ArithOps {
                #[chumsky(format = "$add {lhs}, {rhs} -> {result:type}")]
                Add {
                    result: ResultValue,
                    lhs: Value,
                    rhs: Value,
                },
                #[chumsky(format = "$sub {lhs}, {rhs} -> {result:type}")]
                Sub {
                    result: ResultValue,
                    lhs: Value,
                    rhs: Value,
                },
            }
        };
        insta::assert_snapshot!(generate_pretty_print_code(input));
    }

    #[test]
    fn test_wrapper_namespace_pretty_print() {
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
        insta::assert_snapshot!(generate_pretty_print_code(input));
    }
}
