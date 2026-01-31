//! Code generation for the `HasRecursiveParser` derive macro.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{CollectedField, FieldKind, collect_fields};
use crate::format::{Format, FormatElement, FormatOption};
use crate::generics::GenericsBuilder;

/// Represents an occurrence of a field in the format string.
#[derive(Debug)]
struct FieldOccurrence<'a> {
    /// The collected field info.
    field: &'a CollectedField,
    /// The format option for this occurrence.
    option: FormatOption,
    /// The unique variable name for this occurrence.
    var_name: syn::Ident,
}

/// Generator for the `HasRecursiveParser` trait implementation.
pub struct GenerateHasRecursiveParser {
    crate_path: syn::Path,
}

impl GenerateHasRecursiveParser {
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

    /// Generates the `HasRecursiveParser` implementation.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_generics = self.build_ast_generics(ir_input);
        let crate_path = &self.crate_path;

        // Generate impl for the AST type (generic over Language)
        let ast_parser_impl =
            self.generate_parser_impl(ir_input, &ast_name, &ast_generics, crate_path);

        // Generate impl for the original type (Self as its own Language)
        let original_parser_impl =
            self.generate_original_type_impl(ir_input, &ast_name, crate_path);

        quote! {
            #ast_parser_impl
            #original_parser_impl
        }
    }

    /// Generates the `HasRecursiveParser` impl for the original type.
    /// This allows the original type to be used as its own Language parameter.
    ///
    /// Note: This impl uses `WithAbstractSyntaxTree::AbstractSyntaxTreeNode` instead of
    /// directly referencing the AST type name. This produces a clear compile error if
    /// `WithAbstractSyntaxTree` is not also derived (or implemented manually).
    fn generate_original_type_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        _ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;

        // Build impl generics that include both the lifetimes and the original type parameters
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        // Combine where clauses if both exist
        let combined_where = match (where_clause, impl_where_clause) {
            (Some(orig), Some(impl_wc)) => {
                let mut combined = orig.clone();
                combined
                    .predicates
                    .extend(impl_wc.predicates.iter().cloned());
                Some(combined)
            }
            (Some(wc), None) | (None, Some(wc)) => Some(wc.clone()),
            (None, None) => None,
        };

        // Use the trait's associated type instead of directly referencing the AST type name.
        // This ensures a clear error message if WithAbstractSyntaxTree is not implemented:
        // "the trait `WithAbstractSyntaxTree` is not implemented for `MyType`"
        quote! {
            impl #impl_generics #crate_path::HasRecursiveParser<'tokens, 'src, #original_name #ty_generics>
                for #original_name #ty_generics
            #combined_where
            {
                type Output = <#original_name #ty_generics as #crate_path::WithAbstractSyntaxTree<'tokens, 'src, #original_name #ty_generics>>::AbstractSyntaxTreeNode;

                fn recursive_parser<I>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, Self::Output>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                {
                    <<#original_name #ty_generics as #crate_path::WithAbstractSyntaxTree<
                        'tokens,
                        'src,
                        #original_name #ty_generics,
                    >>::AbstractSyntaxTreeNode as #crate_path::HasRecursiveParser<
                        'tokens,
                        'src,
                        #original_name #ty_generics,
                    >>::recursive_parser(language)
                }
            }
        }
    }

    /// Builds impl generics for the original type's HasRecursiveParser impl.
    /// This includes 'tokens, 'src: 'tokens, plus the original type's type parameters.
    fn build_original_type_impl_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.crate_path).with_lifetimes(&ir_input.generics)
    }

    fn build_ast_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.crate_path).with_language(&ir_input.generics)
    }

    fn generate_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(s) => {
                self.generate_struct_parser(ir_input, &s.0, ast_name, ast_generics, crate_path)
            }
            kirin_derive_core::ir::Data::Enum(e) => {
                self.generate_enum_parser(ir_input, e, ast_name, ast_generics, crate_path)
            }
        }
    }

    fn generate_struct_parser(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();
        let parser_body = match self.build_statement_parser(
            ir_input,
            stmt,
            ast_name,
            ast_generics,
            None,
            crate_path,
        ) {
            Ok(body) => body,
            Err(err) => return err.to_compile_error(),
        };

        quote! {
            impl #impl_generics #crate_path::HasRecursiveParser<'tokens, 'src, Language>
                for #ast_name #ty_generics
            where
                Language: #crate_path::LanguageParser<'tokens, 'src> + 'tokens,
            {
                type Output = Self;

                fn recursive_parser<I>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, <Language as #crate_path::HasRecursiveParser<'tokens, 'src, Language>>::Output>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                    Language: #crate_path::HasRecursiveParser<'tokens, 'src, Language>,
                {
                    use ::chumsky::prelude::*;
                    #parser_body.boxed()
                }
            }
        }
    }

    fn generate_enum_parser(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();

        let mut variant_parsers = Vec::new();
        for variant in &data.variants {
            let parser = self.build_statement_parser(
                ir_input,
                variant,
                ast_name,
                ast_generics,
                Some(&variant.name),
                crate_path,
            );
            match parser {
                Ok(p) => variant_parsers.push(p),
                Err(err) => variant_parsers.push(err.to_compile_error()),
            }
        }

        let combined = if variant_parsers.is_empty() {
            quote! { ::chumsky::prelude::empty().map(|_: ()| unreachable!()) }
        } else {
            variant_parsers
                .into_iter()
                .reduce(|acc, parser| quote! { #acc.or(#parser) })
                .unwrap()
        };

        quote! {
            impl #impl_generics #crate_path::HasRecursiveParser<'tokens, 'src, Language>
                for #ast_name #ty_generics
            where
                Language: #crate_path::LanguageParser<'tokens, 'src> + 'tokens,
            {
                type Output = Self;

                fn recursive_parser<I>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, <Language as #crate_path::HasRecursiveParser<'tokens, 'src, Language>>::Output>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                    Language: #crate_path::HasRecursiveParser<'tokens, 'src, Language>,
                {
                    use ::chumsky::prelude::*;
                    #combined.boxed()
                }
            }
        }
    }

    fn build_statement_parser(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        variant: Option<&syn::Ident>,
        crate_path: &syn::Path,
    ) -> syn::Result<TokenStream> {
        // Check if this is a wrapper variant
        if let Some(wrapper) = &stmt.wraps {
            return self.build_wrapper_parser(ast_name, ast_generics, variant, wrapper, crate_path);
        }

        let format_str = self
            .format_for_statement(ir_input, stmt)
            .ok_or_else(|| syn::Error::new(stmt.name.span(), "missing chumsky format attribute"))?;

        let format = Format::parse(&format_str, None)?;
        let collected = collect_fields(stmt);

        // Build field occurrences - each format field becomes an occurrence
        let occurrences = self.build_field_occurrences(stmt, &format, &collected)?;

        // Build parser chain properly handling the tuple nesting
        let parser_expr = self.build_parser_chain_v2(&format, &occurrences, crate_path)?;

        // Generate pattern matching for the parser output
        let var_names: Vec<_> = occurrences.iter().map(|o| o.var_name.clone()).collect();
        let pattern = self.build_pattern_v2(&var_names);
        let constructor =
            self.ast_constructor_v2(ast_name, variant, &collected, &occurrences, crate_path);

        Ok(quote! {{
            use ::kirin_lexer::Token;
            #parser_expr.map(|#pattern| #constructor)
        }})
    }

    /// Builds field occurrences from the format string.
    /// Each field in the format string becomes an occurrence with a unique variable name.
    fn build_field_occurrences<'a>(
        &self,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        format: &Format<'_>,
        collected: &'a [CollectedField],
    ) -> syn::Result<Vec<FieldOccurrence<'a>>> {
        let map_by_ident = stmt.field_name_to_index();

        // Validate that no fields use Vec or Option collection types.
        // Format strings don't support list/optional syntax, so these must be rejected.
        for field in collected {
            match field.collection {
                kirin_derive_core::ir::fields::Collection::Vec => {
                    let field_name = field
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| format!("field at index {}", field.index));
                    return Err(syn::Error::new(
                        stmt.name.span(),
                        format!(
                            "field '{}' has type Vec<...> which is not supported in format-derived parsers. \
                             Format strings do not define list syntax (separators, delimiters). \
                             Consider using a single-element field or implementing HasRecursiveParser manually.",
                            field_name
                        ),
                    ));
                }
                kirin_derive_core::ir::fields::Collection::Option => {
                    let field_name = field
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| format!("field at index {}", field.index));
                    return Err(syn::Error::new(
                        stmt.name.span(),
                        format!(
                            "field '{}' has type Option<...> which is not supported in format-derived parsers. \
                             Format strings do not define optional syntax. \
                             Consider using a required field or implementing HasRecursiveParser manually.",
                            field_name
                        ),
                    ));
                }
                kirin_derive_core::ir::fields::Collection::Single => {}
            }
        }

        let mut occurrences = Vec::new();

        for elem in format.elements() {
            if let FormatElement::Field(name, opt) = elem {
                let key = name.to_string();
                let index = name
                    .parse::<usize>()
                    .ok()
                    .or_else(|| map_by_ident.get(&key).copied())
                    .ok_or_else(|| {
                        syn::Error::new(
                            stmt.name.span(),
                            format!("unknown field '{}' in format string", name),
                        )
                    })?;

                let field = collected.iter().find(|f| f.index == index).ok_or_else(|| {
                    syn::Error::new(stmt.name.span(), format!("field index {} not found", index))
                })?;

                // Validate that :name and :type options are only used on SSA/Result fields
                if matches!(opt, FormatOption::Name | FormatOption::Type) {
                    let is_ssa_like =
                        matches!(field.kind, FieldKind::SSAValue | FieldKind::ResultValue);
                    if !is_ssa_like {
                        let field_name = field
                            .ident
                            .as_ref()
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| format!("field at index {}", index));
                        let kind_name = match &field.kind {
                            FieldKind::Block => "block",
                            FieldKind::Successor => "successor",
                            FieldKind::Region => "region",
                            FieldKind::Value(_) => "value",
                            FieldKind::SSAValue | FieldKind::ResultValue => unreachable!(),
                        };
                        let option_name = match opt {
                            FormatOption::Name => ":name",
                            FormatOption::Type => ":type",
                            FormatOption::Default => unreachable!(),
                        };
                        return Err(syn::Error::new(
                            stmt.name.span(),
                            format!(
                                "format option '{}' cannot be used on {} field '{}'. \
                                 The :name and :type options are only valid for SSAValue and ResultValue fields.",
                                option_name, kind_name, field_name
                            ),
                        ));
                    }
                }

                // Check for duplicate default occurrences
                if matches!(opt, FormatOption::Default) {
                    let existing_default = occurrences.iter().any(|o: &FieldOccurrence<'_>| {
                        o.field.index == index && matches!(o.option, FormatOption::Default)
                    });
                    if existing_default {
                        let field_name = field
                            .ident
                            .as_ref()
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| format!("field at index {}", index));
                        return Err(syn::Error::new(
                            stmt.name.span(),
                            format!(
                                "field '{}' appears multiple times with default format option. \
                                 Each field can only have one default occurrence. \
                                 Use {{{}:name}} or {{{}:type}} for additional occurrences.",
                                field_name, field_name, field_name
                            ),
                        ));
                    }
                }

                // Generate unique variable name based on field and option
                let var_name = match opt {
                    FormatOption::Name => {
                        let base = field
                            .ident
                            .as_ref()
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| format!("field_{}", index));
                        syn::Ident::new(&format!("{}_name", base), proc_macro2::Span::call_site())
                    }
                    FormatOption::Type => {
                        let base = field
                            .ident
                            .as_ref()
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| format!("field_{}", index));
                        syn::Ident::new(&format!("{}_type", base), proc_macro2::Span::call_site())
                    }
                    FormatOption::Default => {
                        // Since we reject duplicate defaults above, this is the only default occurrence
                        field.ident.clone().unwrap_or_else(|| {
                            syn::Ident::new(
                                &format!("field_{}", index),
                                proc_macro2::Span::call_site(),
                            )
                        })
                    }
                };

                occurrences.push(FieldOccurrence {
                    field,
                    option: opt.clone(),
                    var_name,
                });
            }
        }

        // Validate that all fields are mentioned in the format string.
        // This prevents silent fallback to Default::default() for missing fields.
        for field in collected {
            let is_mentioned = occurrences.iter().any(|o| o.field.index == field.index);
            if !is_mentioned {
                let field_name = field
                    .ident
                    .as_ref()
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| format!("field at index {}", field.index));
                return Err(syn::Error::new(
                    stmt.name.span(),
                    format!(
                        "field '{}' is not mentioned in the format string. \
                         All fields must appear in the format string. \
                         Use {{{}}} or {{{}:name}}/{{{}:type}} to include this field.",
                        field_name, field_name, field_name, field_name
                    ),
                ));
            }
        }

        Ok(occurrences)
    }

    fn build_parser_chain_v2(
        &self,
        format: &Format<'_>,
        occurrences: &[FieldOccurrence<'_>],
        crate_path: &syn::Path,
    ) -> syn::Result<TokenStream> {
        let mut occurrence_iter = occurrences.iter();
        let mut parser_parts: Vec<ParserPart> = Vec::new();

        for elem in format.elements() {
            match elem {
                FormatElement::Token(tokens) => {
                    parser_parts.push(ParserPart::Token(self.token_parser(tokens)));
                }
                FormatElement::Field(_, _) => {
                    let occurrence = occurrence_iter
                        .next()
                        .expect("occurrence sequence mismatch");
                    parser_parts.push(ParserPart::Field(self.field_parser_v2(
                        crate_path,
                        occurrence.field,
                        &occurrence.option,
                    )));
                }
            }
        }

        // Build the parser chain
        if parser_parts.is_empty() {
            return Ok(quote! { ::chumsky::prelude::empty() });
        }

        // Find the first field parser
        let first_field_idx = parser_parts
            .iter()
            .position(|p| matches!(p, ParserPart::Field(_)));

        let mut parser_expr: Option<TokenStream> = None;

        for (i, part) in parser_parts.iter().enumerate() {
            match part {
                ParserPart::Token(tok_parser) => {
                    match &parser_expr {
                        Some(expr) => {
                            parser_expr = Some(quote! { #expr.then_ignore(#tok_parser) });
                        }
                        None => {
                            // Check if there's a field coming after
                            if first_field_idx.is_some() && i < first_field_idx.unwrap() {
                                // Don't set parser_expr yet - we'll use ignore_then
                                continue;
                            } else {
                                // No fields, just use ignored()
                                parser_expr = Some(quote! { #tok_parser });
                            }
                        }
                    }
                }
                ParserPart::Field(field_parser) => {
                    match &parser_expr {
                        Some(expr) => {
                            parser_expr = Some(quote! { #expr.then(#field_parser) });
                        }
                        None => {
                            // Check if there are preceding tokens
                            let preceding_tokens: Vec<_> = parser_parts[..i]
                                .iter()
                                .filter_map(|p| match p {
                                    ParserPart::Token(t) => Some(t.clone()),
                                    _ => None,
                                })
                                .collect();

                            if !preceding_tokens.is_empty() {
                                // Combine preceding tokens
                                let mut combined = preceding_tokens[0].clone();
                                for tok in &preceding_tokens[1..] {
                                    combined = quote! { #combined.then_ignore(#tok) };
                                }
                                parser_expr = Some(quote! { #combined.ignore_then(#field_parser) });
                            } else {
                                parser_expr = Some(field_parser.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(parser_expr.unwrap_or_else(|| quote! { ::chumsky::prelude::empty() }))
    }

    fn build_pattern_v2(&self, var_names: &[syn::Ident]) -> TokenStream {
        if var_names.is_empty() {
            return quote! { _ };
        }

        // Build nested tuple pattern for fields
        let mut iter = var_names.iter();
        let first = iter.next().unwrap();
        let mut pattern = quote! { #first };

        for ident in iter {
            pattern = quote! { (#pattern, #ident) };
        }

        pattern
    }

    /// Generate field parser based on field kind and format option.
    fn field_parser_v2(
        &self,
        crate_path: &syn::Path,
        field: &CollectedField,
        opt: &FormatOption,
    ) -> TokenStream {
        let base = match &field.kind {
            FieldKind::SSAValue => match opt {
                FormatOption::Name => quote! { #crate_path::nameof_ssa() },
                FormatOption::Type => quote! { #crate_path::typeof_ssa::<_, Language>() },
                FormatOption::Default => quote! { #crate_path::ssa_value::<_, Language>() },
            },
            FieldKind::ResultValue => match opt {
                FormatOption::Name => quote! { #crate_path::nameof_ssa() },
                FormatOption::Type => quote! { #crate_path::typeof_ssa::<_, Language>() },
                FormatOption::Default => quote! { #crate_path::result_value::<_, Language>() },
            },
            FieldKind::Block => {
                quote! { #crate_path::block::<_, Language>(language.clone()) }
            }
            FieldKind::Successor => {
                quote! { #crate_path::block_label() }
            }
            FieldKind::Region => {
                quote! { #crate_path::region::<_, Language>(language.clone()) }
            }
            FieldKind::Value(ty) => {
                quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::parser() }
            }
        };

        match field.collection {
            kirin_derive_core::ir::fields::Collection::Single => base,
            kirin_derive_core::ir::fields::Collection::Vec => {
                quote! { #base.repeated().collect() }
            }
            kirin_derive_core::ir::fields::Collection::Option => {
                quote! { #base.or_not() }
            }
        }
    }

    /// Generate AST constructor that combines field occurrences.
    fn ast_constructor_v2(
        &self,
        ast_name: &syn::Ident,
        variant: Option<&syn::Ident>,
        collected: &[CollectedField],
        occurrences: &[FieldOccurrence<'_>],
        crate_path: &syn::Path,
    ) -> TokenStream {
        // Group occurrences by field index
        let mut field_occurrences: HashMap<usize, Vec<&FieldOccurrence>> = HashMap::new();
        for occ in occurrences {
            field_occurrences
                .entry(occ.field.index)
                .or_default()
                .push(occ);
        }

        // Check if we have named fields
        let has_named = collected.first().and_then(|f| f.ident.as_ref()).is_some();

        if has_named {
            let assigns = collected.iter().map(|field| {
                let name = field.ident.as_ref().unwrap();
                let value = self.build_field_value(field, &field_occurrences, crate_path);
                quote! { #name: #value }
            });
            match variant {
                Some(v) => quote! { #ast_name::#v { #(#assigns),* } },
                None => quote! { #ast_name { #(#assigns),* } },
            }
        } else {
            let values = collected
                .iter()
                .map(|field| self.build_field_value(field, &field_occurrences, crate_path));
            match variant {
                Some(v) => quote! { #ast_name::#v ( #(#values),* ) },
                None => quote! { #ast_name ( #(#values),* ) },
            }
        }
    }

    /// Build the value expression for a field based on its occurrences.
    fn build_field_value(
        &self,
        field: &CollectedField,
        field_occurrences: &HashMap<usize, Vec<&FieldOccurrence>>,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let occs = field_occurrences.get(&field.index);

        match occs {
            None => {
                // Field not in format string - this should be caught by validation in
                // build_field_occurrences, so this case is unreachable in practice.
                unreachable!(
                    "field '{}' not in format string - this should have been caught earlier",
                    field
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| format!("index {}", field.index))
                )
            }
            Some(occs) if occs.len() == 1 => {
                // Single occurrence - use the variable directly or wrap if needed
                let occ = occs[0];
                let var = &occ.var_name;

                match (&field.kind, &occ.option) {
                    // ResultValue with only :name - need to create ResultValue with None type
                    (FieldKind::ResultValue, FormatOption::Name) => {
                        quote! {
                            #crate_path::ResultValue {
                                name: #crate_path::Spanned { value: #var.name, span: #var.span },
                                ty: None,
                            }
                        }
                    }
                    // ResultValue with only :type - need to create ResultValue with empty name
                    // This is unusual but we handle it
                    (FieldKind::ResultValue, FormatOption::Type) => {
                        quote! {
                            #crate_path::ResultValue {
                                name: #crate_path::Spanned { value: "", span: #var.span },
                                ty: Some(#var.ty.clone()),
                            }
                        }
                    }
                    // Default case - variable is already the correct type
                    _ => quote! { #var },
                }
            }
            Some(occs) => {
                // Multiple occurrences - need to combine them
                // Find :name and :type occurrences
                let name_occ = occs.iter().find(|o| matches!(o.option, FormatOption::Name));
                let type_occ = occs.iter().find(|o| matches!(o.option, FormatOption::Type));

                match (&field.kind, name_occ, type_occ) {
                    // ResultValue with both :name and :type
                    (FieldKind::ResultValue, Some(name), Some(ty)) => {
                        let name_var = &name.var_name;
                        let ty_var = &ty.var_name;
                        quote! {
                            #crate_path::ResultValue {
                                name: #crate_path::Spanned { value: #name_var.name, span: #name_var.span },
                                ty: Some(#ty_var.ty.clone()),
                            }
                        }
                    }
                    // SSAValue with both :name and :type
                    (FieldKind::SSAValue, Some(name), Some(ty)) => {
                        let name_var = &name.var_name;
                        let ty_var = &ty.var_name;
                        quote! {
                            #crate_path::SSAValue {
                                name: #crate_path::Spanned { value: #name_var.name, span: #name_var.span },
                                ty: Some(#ty_var.ty.clone()),
                            }
                        }
                    }
                    // Fallback - use the first occurrence
                    _ => {
                        let var = &occs[0].var_name;
                        quote! { #var }
                    }
                }
            }
        }
    }

    fn build_wrapper_parser(
        &self,
        ast_name: &syn::Ident,
        _ast_generics: &syn::Generics,
        variant: Option<&syn::Ident>,
        wrapper: &kirin_derive_core::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> syn::Result<TokenStream> {
        let wrapped_ty = &wrapper.ty;

        let constructor = match variant {
            Some(v) => quote! { #ast_name::#v },
            None => quote! { #ast_name },
        };

        Ok(quote! {
            <#wrapped_ty as #crate_path::HasRecursiveParser<'tokens, 'src, Language>>::recursive_parser(language.clone())
                .map(|inner| #constructor(inner))
        })
    }

    fn format_for_statement(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
    ) -> Option<String> {
        stmt.extra_attrs
            .format
            .clone()
            .or(stmt.attrs.format.clone())
            .or(ir_input.extra_attrs.format.clone())
    }

    fn token_parser(&self, tokens: &[kirin_lexer::Token<'_>]) -> TokenStream {
        let mut iter = tokens.iter();
        let Some(first) = iter.next() else {
            return quote! { ::chumsky::prelude::empty().ignored() };
        };
        let mut parser = quote! { ::chumsky::prelude::just(#first) };
        for tok in iter {
            parser = quote! { #parser.then_ignore(::chumsky::prelude::just(#tok)) };
        }
        parser
    }
}

enum ParserPart {
    Token(TokenStream),
    Field(TokenStream),
}
