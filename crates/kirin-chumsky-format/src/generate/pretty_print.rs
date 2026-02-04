//! Code generation for the `PrettyPrint` derive macro.
//!
//! This generates `PrettyPrint` implementations for dialect types based on their
//! `chumsky(format = "...")` attributes. The generated printer mirrors the parser,
//! ensuring roundtrip compatibility.

use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use kirin_derive_core::ir::fields::FieldInfo;

use crate::field_kind::{FieldKind, collect_fields};
use crate::format::{Format, FormatElement};
use kirin_lexer::Token;

use super::{GeneratorConfig, generate_enum_match};

/// Generator for the `PrettyPrint` trait implementation.
pub struct GeneratePrettyPrint {
    /// Path to the kirin_prettyless crate
    prettyless_path: syn::Path,
}

impl GeneratePrettyPrint {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        // Get the prettyless crate path from extra_attrs first (e.g., #[chumsky(crate = ...)])
        // or fall back to attrs, then derive the prettyless path
        let crate_path = ir_input
            .extra_attrs
            .crate_path
            .as_ref()
            .or(ir_input.attrs.crate_path.as_ref());

        let prettyless_path = crate_path
            .map(|p| {
                // If user specified a crate path like `kirin::parsers`, derive prettyless as sibling
                // e.g., `kirin::parsers` -> `kirin::pretty`
                let mut segments = p.segments.clone();
                if let Some(last) = segments.last_mut() {
                    if last.ident == "parsers" {
                        last.ident = syn::Ident::new("pretty", last.ident.span());
                        return syn::Path {
                            leading_colon: p.leading_colon,
                            segments,
                        };
                    }
                }
                // Otherwise fall back to default
                syn::parse_quote!(::kirin::pretty)
            })
            .unwrap_or_else(|| syn::parse_quote!(::kirin::pretty));
        Self { prettyless_path }
    }

    /// Generates the `PrettyPrint` implementation.
    ///
    /// Generates `impl PrettyPrint for Self` with a method generic over `L: Dialect`.
    /// This avoids the trait resolution overflow that occurred with the old
    /// `impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Block` pattern.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        // For wrapper structs, forward to the wrapped type's PrettyPrint
        if let kirin_derive_core::ir::Data::Struct(data) = &ir_input.data {
            if let Some(wrapper) = &data.0.wraps {
                return self.generate_wrapper_struct_pretty_print(ir_input, wrapper);
            }
        }

        self.generate_pretty_print(ir_input)
    }

    /// Generates `impl PrettyPrint for Self`.
    fn generate_pretty_print(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        let dialect_name = &ir_input.name;
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();
        let prettyless_path = &self.prettyless_path;
        let config = GeneratorConfig::new(ir_input);
        let ir_path = &config.ir_path;

        // Generate the pretty print body based on struct/enum
        let print_body = match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(s) => {
                self.generate_struct_print(ir_input, &s.0, dialect_name, None)
            }
            kirin_derive_core::ir::Data::Enum(e) => self.generate_enum_print(ir_input, e, dialect_name),
        };

        let (impl_generics, _, _) = ir_input.generics.split_for_impl();

        // The trait method has fixed bounds: L: Dialect + PrettyPrint, L::TypeLattice: Display
        // All implementations must match these bounds
        quote! {
            impl #impl_generics #prettyless_path::PrettyPrint
                for #dialect_name #ty_generics
            #where_clause
            {
                fn pretty_print<'a, __L: #ir_path::Dialect + #prettyless_path::PrettyPrint>(
                    &self,
                    doc: &'a #prettyless_path::Document<'a, __L>,
                ) -> #prettyless_path::ArenaDoc<'a>
                where
                    __L::TypeLattice: ::core::fmt::Display,
                {
                    use #prettyless_path::DocAllocator;
                    #print_body
                }
            }
        }
    }

    /// Generates the `PrettyPrint` impl for wrapper structs.
    ///
    /// For wrapper structs, we delegate to the wrapped type's PrettyPrint implementation.
    fn generate_wrapper_struct_pretty_print(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_core::ir::fields::Wrapper,
    ) -> TokenStream {
        let dialect_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;
        let prettyless_path = &self.prettyless_path;
        let config = GeneratorConfig::new(ir_input);
        let ir_path = &config.ir_path;

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();
        let (impl_generics, _, _) = ir_input.generics.split_for_impl();

        // The wrapped type needs PrettyPrint bound
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
            impl #impl_generics #prettyless_path::PrettyPrint
                for #dialect_name #ty_generics
            #final_where
            {
                fn pretty_print<'a, __L: #ir_path::Dialect + #prettyless_path::PrettyPrint>(
                    &self,
                    doc: &'a #prettyless_path::Document<'a, __L>,
                ) -> #prettyless_path::ArenaDoc<'a>
                where
                    __L::TypeLattice: ::core::fmt::Display,
                {
                    // Delegate to the wrapped type's PrettyPrint
                    let inner = &self.0;
                    #prettyless_path::PrettyPrint::pretty_print(inner, doc)
                }
            }
        }
    }

    fn generate_struct_print(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        dialect_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
    ) -> TokenStream {
        let (pattern, print_expr) = self.build_print_components(ir_input, stmt, dialect_name, variant_name);

        quote! {
            let #pattern = self;
            #print_expr
        }
    }

    /// Builds the pattern and print expression for a statement.
    ///
    /// This is shared between struct and variant print generation.
    fn build_print_components(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        dialect_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
    ) -> (TokenStream, TokenStream) {
        // Use the shared helper that checks both #[chumsky(format = ...)] and #[kirin(format = ...)]
        let format_str = super::format_for_statement(ir_input, stmt)
            .expect("Statement must have format string");

        let format = Format::parse(&format_str, None).expect("Format string should be valid");

        let collected = collect_fields(stmt);
        let field_map = build_field_map(&collected);
        let bindings = stmt.field_bindings("f");
        let fields = &bindings.field_idents;

        let print_expr = self.generate_format_print(&format, &field_map, &collected, fields);

        let pattern = if bindings.is_empty() {
            // Empty variant - no parens for tuple style, {} for named style
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

    /// Generates enum print code.
    ///
    /// Wrapper variants delegate to the wrapped type's PrettyPrint implementation.
    fn generate_enum_print(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
        dialect_name: &syn::Ident,
    ) -> TokenStream {
        let prettyless_path = &self.prettyless_path;

        generate_enum_match(
            dialect_name,
            data,
            // Wrapper handler - delegate to wrapped type
            |_name, _wrapper| {
                quote! {
                    #prettyless_path::PrettyPrint::pretty_print(inner, doc)
                }
            },
            // Regular variant handler
            |name, variant| self.generate_variant_print(ir_input, variant, dialect_name, name),
            None, // No marker for dialect types
        )
    }

    /// Generates pretty print code for a single enum variant.
    fn generate_variant_print(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        variant: &kirin_derive_core::ir::Statement<ChumskyLayout>,
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

    fn generate_format_print(
        &self,
        format: &Format,
        field_map: &IndexMap<String, (usize, &FieldInfo<ChumskyLayout>)>,
        _collected: &[FieldInfo<ChumskyLayout>],
        field_vars: &[syn::Ident],
    ) -> TokenStream {
        let prettyless_path = &self.prettyless_path;
        let elements = format.elements();

        // Build the document expression by combining format elements
        let mut parts: Vec<TokenStream> = Vec::new();

        for (i, elem) in elements.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == elements.len() - 1;
            let prev_is_field = i > 0 && matches!(elements[i - 1], FormatElement::Field(_, _));
            let next_is_field = !is_last && matches!(elements[i + 1], FormatElement::Field(_, _));

            match elem {
                FormatElement::Token(tokens) => {
                    // Convert tokens to text with proper spacing
                    let text = tokens_to_string_with_spacing(tokens, prev_is_field, next_is_field);
                    parts.push(quote! { doc.text(#text) });
                }
                FormatElement::Field(name, opt) => {
                    // Look up the field by name
                    let name_str = name.to_string();
                    if let Some((idx, field)) = field_map.get(&name_str) {
                        let var = &field_vars[*idx];
                        let var_ref = quote! { #var };

                        let kind = FieldKind::from_field_info(field);
                        let print_expr = kind.print_expr(prettyless_path, &var_ref, opt);

                        // Add space before field if preceded by another field (no Token between)
                        if !is_first && prev_is_field {
                            parts.push(quote! { doc.text(" ") });
                        }

                        parts.push(print_expr);
                    }
                }
            }
        }

        // Combine parts with + operator
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
}

/// Build a map from field name/index (string) to (index, FieldInfo)
///
/// For named fields, both the field name and its index are added as keys.
/// This allows format strings to use either `{field_name}` or `{0}` syntax.
fn build_field_map(collected: &[FieldInfo<ChumskyLayout>]) -> IndexMap<String, (usize, &FieldInfo<ChumskyLayout>)> {
    let mut map = IndexMap::new();
    for (idx, field) in collected.iter().enumerate() {
        // Always add the index as a key (for {0}, {1}, etc. syntax)
        map.insert(field.index.to_string(), (idx, field));

        // Also add the name if it's a named field (for {field_name} syntax)
        if let Some(ident) = &field.ident {
            map.insert(ident.to_string(), (idx, field));
        }
    }
    map
}

/// Convert a sequence of tokens to a string for printing with proper spacing.
///
/// - `add_leading_space`: Add a space before the first token
/// - `add_trailing_space`: Add a space after the last token
fn tokens_to_string_with_spacing(
    tokens: &[Token],
    add_leading_space: bool,
    add_trailing_space: bool,
) -> String {
    let mut result = String::new();

    // Add leading space if preceded by a field
    if add_leading_space && !tokens.is_empty() {
        // Check if the first token is a punctuation that typically doesn't want leading space
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
        // Use Display impl for most tokens, special-case escaped braces
        match token {
            Token::EscapedLBrace => result.push('{'),
            Token::EscapedRBrace => result.push('}'),
            other => result.push_str(&other.to_string()),
        }
    }

    // Add trailing space if followed by a field
    if add_trailing_space && !tokens.is_empty() {
        // Check if the last token is a punctuation that typically doesn't want trailing space
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
