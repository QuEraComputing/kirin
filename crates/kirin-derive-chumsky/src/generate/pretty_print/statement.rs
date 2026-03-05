use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::quote;

use crate::PrettyPrintLayout;
use kirin_derive_toolkit::ir::fields::FieldInfo;

use crate::field_kind::{FieldKind, collect_fields};
use crate::format::{Format, FormatElement};

use crate::generate::generate_enum_match;

use super::GeneratePrettyPrint;
use super::helpers::{build_field_map, tokens_to_string_with_spacing};

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

        quote! {
            #[automatically_derived]
            impl #impl_generics #prettyless_path::PrettyPrint
                for #dialect_name #ty_generics
            #where_clause
            {
                fn pretty_print<'a, __L: #ir_path::Dialect + #prettyless_path::PrettyPrint>(
                    &self,
                    doc: &'a #prettyless_path::Document<'a, __L>,
                ) -> #prettyless_path::ArenaDoc<'a>
                where
                    __L::Type: ::core::fmt::Display,
                {
                    use #prettyless_path::DocAllocator;
                    #print_body
                }
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
                fn pretty_print<'a, __L: #ir_path::Dialect + #prettyless_path::PrettyPrint>(
                    &self,
                    doc: &'a #prettyless_path::Document<'a, __L>,
                ) -> #prettyless_path::ArenaDoc<'a>
                where
                    __L::Type: ::core::fmt::Display,
                {
                    let inner = &self.0;
                    #prettyless_path::PrettyPrint::pretty_print(inner, doc)
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
        let format_str = crate::generate::format_for_statement(ir_input, stmt)
            .expect("Statement must have format string");

        let format = Format::parse(&format_str, None).expect("Format string should be valid");

        let collected = collect_fields(stmt);
        let field_map = build_field_map(&collected);
        let bindings = stmt.field_bindings("f");
        let fields = &bindings.field_idents;

        let print_expr = self.generate_format_print(&format, &field_map, &collected, fields);

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
        let prettyless_path = &self.prettyless_path;

        generate_enum_match(
            dialect_name,
            data,
            |_name, _wrapper| {
                quote! {
                    #prettyless_path::PrettyPrint::pretty_print(inner, doc)
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
    ) -> TokenStream {
        let prettyless_path = &self.prettyless_path;
        let elements = format.elements();

        let mut parts: Vec<TokenStream> = Vec::new();

        for (i, elem) in elements.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == elements.len() - 1;
            let prev_is_field = i > 0 && matches!(elements[i - 1], FormatElement::Field(_, _));
            let next_is_field = !is_last && matches!(elements[i + 1], FormatElement::Field(_, _));

            match elem {
                FormatElement::Token(tokens) => {
                    let text = tokens_to_string_with_spacing(tokens, prev_is_field, next_is_field);
                    parts.push(quote! { doc.text(#text) });
                }
                FormatElement::Field(name, opt) => {
                    let name_str = name.to_string();
                    if let Some((idx, field)) = field_map.get(&name_str) {
                        let var = &field_vars[*idx];
                        let var_ref = quote! { #var };

                        let kind = FieldKind::from_field_info(field);
                        let print_expr = kind.print_expr(prettyless_path, &var_ref, opt);

                        if !is_first && prev_is_field {
                            parts.push(quote! { doc.text(" ") });
                        }

                        parts.push(print_expr);
                    }
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
}
