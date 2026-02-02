//! Code generation for the `HasDialectParser` derive macro.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::{CollectedField, collect_fields};
use crate::format::{Format, FormatElement, FormatOption};
use crate::generics::GenericsBuilder;

use super::{GeneratorConfig, collect_all_value_types_needing_bounds, format_for_statement};

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

/// Generator for the `HasDialectParser` trait implementation.
pub struct GenerateHasDialectParser {
    config: GeneratorConfig,
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

    /// Generates the `HasParser` impl for the original type.
    /// This provides the `parser()` method that sets up recursive parsing.
    fn generate_has_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let type_lattice = &ir_input.attrs.type_lattice;

        // Build impl generics that include both the lifetimes and the original type parameters
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        // Combine where clauses if both exist, and add TypeLattice: HasParser bound
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

        // Add the TypeLattice: HasParser bound needed for type annotations
        // This bound is required because HasDialectParser sets TypeAST = <TypeLattice as HasParser>::Output,
        // and parsers like ssa_value require TypeAST: HasParser
        // The 'tokens bound is required since the type is used in AST with that lifetime
        let type_lattice_bound: syn::WherePredicate = syn::parse_quote! {
            #type_lattice: #crate_path::HasParser<'tokens, 'src> + 'tokens
        };

        // Add HasParser bounds for Value field types containing type parameters
        // These types only need HasParser<'tokens, 'src> + 'tokens bound (no Output restriction)
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds: Vec<syn::WherePredicate> = value_types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens })
            .collect();

        let where_clause = match combined_where {
            Some(mut wc) => {
                wc.predicates.push(type_lattice_bound);
                wc.predicates.extend(value_type_bounds);
                quote! { #wc }
            }
            None => {
                let all_bounds = std::iter::once(type_lattice_bound)
                    .chain(value_type_bounds)
                    .collect::<Vec<_>>();
                quote! { where #(#all_bounds),* }
            }
        };

        // The AST type for this dialect
        let ast_type = self.build_ast_type_reference(ir_input, ast_name);

        quote! {
            impl #impl_generics #crate_path::HasParser<'tokens, 'src> for #original_name #ty_generics
            #where_clause
            {
                type Output = #ast_type;

                fn parser<I>() -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                {
                    use #crate_path::chumsky::prelude::*;
                    #crate_path::chumsky::recursive::recursive(|language| {
                        <#original_name #ty_generics as #crate_path::HasDialectParser<
                            'tokens,
                            'src,
                            #original_name #ty_generics,
                        >>::recursive_parser(language)
                    }).boxed()
                }
            }
        }
    }

    /// Generates the `HasDialectParser` impl for the dialect type.
    ///
    /// Only the dialect type implements `HasDialectParser`. The AST type is just the Output.
    /// The impl is generic over `Language` to allow this dialect to be embedded in a larger
    /// language composition rather than always being the top-level language.
    fn generate_dialect_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let type_lattice = &ir_input.attrs.type_lattice;
        let ir_path = &self.config.ir_path;

        // Build impl generics that include lifetimes, original type parameters, and Language
        // Language is added without bounds here; the Dialect bound is in the where clause
        let impl_generics = GenericsBuilder::new(&self.config.ir_path).with_language_unbounded(&ir_input.generics);
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

        // Add HasParser bounds for Value field types containing type parameters
        // These types need HasParser<'tokens, 'src> + 'tokens bound
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds: Vec<syn::WherePredicate> = value_types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens })
            .collect();

        // Add TypeLattice: HasParser bound (needed for TypeAST = <TypeLattice as HasParser>::Output)
        // The 'tokens bound is required since the type is used in AST with that lifetime
        let type_lattice_bound: syn::WherePredicate = syn::parse_quote! {
            #type_lattice: #crate_path::HasParser<'tokens, 'src> + 'tokens
        };

        // Build the final where clause with all bounds including Language: Dialect + 'tokens
        // The HasDialectParser bound is added to the method's where clause, not the impl
        let language_dialect_bound: syn::WherePredicate = syn::parse_quote! {
            Language: #ir_path::Dialect + 'tokens
        };

        let final_where = {
            let mut wc = match combined_where {
                Some(wc) => wc,
                None => syn::WhereClause {
                    where_token: syn::token::Where::default(),
                    predicates: syn::punctuated::Punctuated::new(),
                },
            };
            wc.predicates.push(language_dialect_bound);
            wc.predicates.push(type_lattice_bound);
            wc.predicates.extend(value_type_bounds);
            wc
        };

        // Generate parser body based on struct/enum
        let parser_body = self.generate_dialect_parser_body(ir_input, ast_name, crate_path);

        // The AST type for this dialect, using generic Language parameter
        let ast_type = self.build_ast_type_reference_generic(ir_input, ast_name);

        // The Language's output type (for the recursive parser argument)
        let language_output = quote! { <Language as #crate_path::HasDialectParser<'tokens, 'src, Language>>::Output };

        quote! {
            impl #impl_generics #crate_path::HasDialectParser<'tokens, 'src, Language>
                for #original_name #ty_generics
            #final_where
            {
                type Output = #ast_type;
                // TypeAST is the output of parsing the type lattice via HasParser
                type TypeAST = <#type_lattice as #crate_path::HasParser<'tokens, 'src>>::Output;

                #[inline]
                fn recursive_parser<I>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, #language_output>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                    Language: #crate_path::HasDialectParser<'tokens, 'src, Language>,
                {
                    use #crate_path::chumsky::prelude::*;
                    // SAFETY: The transmute converts between two identical types:
                    // - #ast_type (the concrete AST type with explicit lifetimes)
                    // - Self::Output (defined as `type Output = #ast_type` above)
                    //
                    // This transmute is necessary because Rust's type system treats associated
                    // types as opaque during type checking. Even though `type Output = #ast_type`
                    // is defined in this impl block, Rust cannot unify the concrete type with
                    // `Self::Output` for type inference purposes. The types are identical by
                    // construction, so this transmute is safe.
                    let parser: #crate_path::BoxedParser<'tokens, 'src, I, #ast_type> = #parser_body.boxed();
                    unsafe { ::core::mem::transmute(parser) }
                }
            }
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
        let ast_generics = self.config.build_ast_generics(ir_input);
        match self.build_statement_parser(ir_input, stmt, ast_name, &ast_generics, None, crate_path)
        {
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
        let ast_generics = self.config.build_ast_generics(ir_input);
        let mut variant_parsers = Vec::new();
        for variant in &data.variants {
            let parser = self.build_statement_parser(
                ir_input,
                variant,
                ast_name,
                &ast_generics,
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

    /// Builds impl generics for the original type's HasDialectParser impl.
    fn build_original_type_impl_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.config.ir_path).with_lifetimes(&ir_input.generics)
    }

    /// Builds the fully-qualified AST type reference with the dialect type as Language.
    ///
    /// AST types have generics: `<'tokens, 'src, [original type params], Language>`
    /// This returns: `ASTName<'tokens, 'src, T, L, ..., DialectType<T, L, ...>>`
    ///
    /// Use this for `HasParser::Output` where Language = Self.
    fn build_ast_type_reference(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let (_, ty_generics, _) = ir_input.generics.split_for_impl();
        let dialect_type = quote! { #original_name #ty_generics };

        self.build_ast_type_reference_with_language(ir_input, ast_name, &dialect_type)
    }

    /// Builds the fully-qualified AST type reference with a generic Language parameter.
    ///
    /// AST types have generics: `<'tokens, 'src, [original type params], Language>`
    /// This returns: `ASTName<'tokens, 'src, T, L, ..., Language>`
    ///
    /// Use this for `HasDialectParser::Output` where Language is a generic parameter.
    fn build_ast_type_reference_generic(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let language = quote! { Language };
        self.build_ast_type_reference_with_language(ir_input, ast_name, &language)
    }

    /// Helper to build AST type reference with a specific Language type.
    fn build_ast_type_reference_with_language(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        language_type: &TokenStream,
    ) -> TokenStream {
        // Extract just the type parameters from the original generics (not lifetimes)
        let type_params: Vec<_> = ir_input
            .generics
            .params
            .iter()
            .filter_map(|p| {
                if let syn::GenericParam::Type(tp) = p {
                    let ident = &tp.ident;
                    Some(quote! { #ident })
                } else {
                    None
                }
            })
            .collect();

        // AST generics are <'tokens, 'src, [original type params], Language>
        if type_params.is_empty() {
            quote! { #ast_name<'tokens, 'src, #language_type> }
        } else {
            quote! { #ast_name<'tokens, 'src, #(#type_params,)* #language_type> }
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
        // Build dialect type (e.g., `TestLang` or `MyDialect<T>`)
        let original_name = &ir_input.name;
        let (_, ty_generics, _) = ir_input.generics.split_for_impl();
        let dialect_type = quote! { #original_name #ty_generics };

        // Check if this is a wrapper variant
        if let Some(wrapper) = &stmt.wraps {
            return self.build_wrapper_parser(
                ir_input,
                ast_name,
                ast_generics,
                variant,
                wrapper,
                crate_path,
                &dialect_type,
            );
        }

        let format_str = format_for_statement(ir_input, stmt)
            .ok_or_else(|| syn::Error::new(stmt.name.span(), "missing chumsky format attribute"))?;

        let format = Format::parse(&format_str, None)?;
        let collected = collect_fields(stmt);

        // Build field occurrences - each format field becomes an occurrence
        let occurrences = self.build_field_occurrences(stmt, &format, &collected)?;

        // Get the type lattice for type annotation parsers
        let type_lattice = &ir_input.attrs.type_lattice;

        // Build parser chain properly handling the tuple nesting
        let parser_expr = self.build_parser_chain_v2(
            &format,
            &occurrences,
            crate_path,
            &dialect_type,
            ast_name,
            type_lattice,
        )?;

        // Generate pattern matching for the parser output
        let var_names: Vec<_> = occurrences.iter().map(|o| o.var_name.clone()).collect();
        let pattern = self.build_pattern_v2(&var_names);
        let constructor =
            self.ast_constructor_v2(ast_name, variant, &collected, &occurrences, crate_path);

        // Use explicit return type annotation to pin the lifetimes correctly.
        // Without this, Rust would infer anonymous lifetimes '_ for the constructor.
        // Use generic Language since this is inside HasDialectParser::recursive_parser.
        let return_type = self.build_ast_type_reference_generic(ir_input, ast_name);
        Ok(quote! {{
            use #crate_path::Token;
            #parser_expr.map(|#pattern| -> #return_type { #constructor })
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
                    return Err(syn::Error::new(
                        stmt.name.span(),
                        format!(
                            "field '{}' has type Vec<...> which is not supported in format-derived parsers. \
                             Format strings do not define list syntax (separators, delimiters). \
                             Consider using a single-element field or implementing HasDialectParser manually.",
                            field
                        ),
                    ));
                }
                kirin_derive_core::ir::fields::Collection::Option => {
                    return Err(syn::Error::new(
                        stmt.name.span(),
                        format!(
                            "field '{}' has type Option<...> which is not supported in format-derived parsers. \
                             Format strings do not define optional syntax. \
                             Consider using a required field or implementing HasDialectParser manually.",
                            field
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
                if matches!(opt, FormatOption::Name | FormatOption::Type)
                    && !field.kind.supports_name_type_options()
                {
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
                            option_name,
                            field.kind.name(),
                            field
                        ),
                    ));
                }

                // Check for duplicate default occurrences
                if matches!(opt, FormatOption::Default) {
                    let existing_default = occurrences.iter().any(|o: &FieldOccurrence<'_>| {
                        o.field.index == index && matches!(o.option, FormatOption::Default)
                    });
                    if existing_default {
                        return Err(syn::Error::new(
                            stmt.name.span(),
                            format!(
                                "field '{}' appears multiple times with default format option. \
                                 Each field can only have one default occurrence. \
                                 Use {{{}:name}} or {{{}:type}} for additional occurrences.",
                                field, field, field
                            ),
                        ));
                    }
                }

                // Generate unique variable name based on field and option
                let var_name = match opt {
                    FormatOption::Name => {
                        syn::Ident::new(&format!("{}_name", field), proc_macro2::Span::call_site())
                    }
                    FormatOption::Type => {
                        syn::Ident::new(&format!("{}_type", field), proc_macro2::Span::call_site())
                    }
                    FormatOption::Default => {
                        // Since we reject duplicate defaults above, this is the only default occurrence
                        field.ident.clone().unwrap_or_else(|| {
                            syn::Ident::new(&format!("{}", field), proc_macro2::Span::call_site())
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

        // Validate that all fields are mentioned in the format string,
        // unless they have a default value specified via #[kirin(default = ...)].
        for field in collected {
            let is_mentioned = occurrences.iter().any(|o| o.field.index == field.index);
            if !is_mentioned && field.default.is_none() {
                return Err(syn::Error::new(
                    stmt.name.span(),
                    format!(
                        "field '{}' is not mentioned in the format string. \
                         All fields must appear in the format string unless they have a default value. \
                         Use {{{}}} or {{{}:name}}/{{{}:type}} to include this field, \
                         or add #[kirin(default)] or #[kirin(default = expr)] to provide a default value.",
                        field, field, field, field
                    ),
                ));
            }
        }

        // Validate that SSAValue/ResultValue fields have at least {field} or {field:name}.
        // These field types require a name to be parsed; only having {field:type} is insufficient.
        for field in collected {
            if field.kind.supports_name_type_options() {
                let has_name_occurrence = occurrences.iter().any(|o| {
                    o.field.index == field.index
                        && matches!(o.option, FormatOption::Default | FormatOption::Name)
                });
                if !has_name_occurrence {
                    return Err(syn::Error::new(
                        stmt.name.span(),
                        format!(
                            "SSA/Result field '{}' must have {{{}}} or {{{}:name}} in the format string. \
                             Using only {{{}:type}} is not sufficient because the name cannot be inferred.",
                            field, field, field, field
                        ),
                    ));
                }
            }
        }

        Ok(occurrences)
    }

    fn build_parser_chain_v2(
        &self,
        format: &Format<'_>,
        occurrences: &[FieldOccurrence<'_>],
        crate_path: &syn::Path,
        dialect_type: &TokenStream,
        ast_name: &syn::Ident,
        type_lattice: &syn::Path,
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
                        dialect_type,
                        ast_name,
                        type_lattice,
                    )));
                }
            }
        }

        // Build the parser chain
        if parser_parts.is_empty() {
            return Ok(quote! { #crate_path::chumsky::prelude::empty() });
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

        Ok(parser_expr.unwrap_or_else(|| quote! { #crate_path::chumsky::prelude::empty() }))
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
        dialect_type: &TokenStream,
        ast_name: &syn::Ident,
        type_lattice: &syn::Path,
    ) -> TokenStream {
        let base = field
            .kind
            .parser_expr(crate_path, opt, dialect_type, ast_name, type_lattice);
        field.collection.wrap_parser(base)
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

        // Filter to only fields that should be in the AST:
        // - Fields that are in the format string (have occurrences), OR
        // - Fields that don't have a default value
        let ast_fields: Vec<_> = collected
            .iter()
            .filter(|f| field_occurrences.contains_key(&f.index) || f.default.is_none())
            .collect();

        // Check if we have named fields
        let has_named = ast_fields.first().and_then(|f| f.ident.as_ref()).is_some();

        if has_named {
            let assigns = ast_fields.iter().map(|field| {
                let name = field.ident.as_ref().unwrap();
                let value = self.build_field_value(field, &field_occurrences, crate_path);
                quote! { #name: #value }
            });
            match variant {
                Some(v) => quote! { #ast_name::#v { #(#assigns),* } },
                // For named structs (not enum variants), add the _marker field
                None => quote! { #ast_name { #(#assigns,)* _marker: ::core::marker::PhantomData } },
            }
        } else {
            // For tuple fields, sort by original index to match AST struct definition order
            let mut sorted_ast_fields: Vec<_> = ast_fields.clone();
            sorted_ast_fields.sort_by_key(|f| f.index);

            let values = sorted_ast_fields
                .iter()
                .map(|field| self.build_field_value(field, &field_occurrences, crate_path));
            match variant {
                Some(v) => quote! { #ast_name::#v ( #(#values),* ) },
                // For tuple structs (not enum variants), add PhantomData at the end
                None => quote! { #ast_name ( #(#values,)* ::core::marker::PhantomData ) },
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
                )
            }
            Some(occs) if occs.len() == 1 => {
                // Single occurrence - use the variable directly or wrap if needed
                let occ = occs[0];
                let var = &occ.var_name;

                match &occ.option {
                    // SSA/Result with only :name - need to create value with None type
                    FormatOption::Name => field
                        .kind
                        .construct_from_name_only(crate_path, var)
                        .unwrap_or_else(|| quote! { #var }),
                    // :type only should have been caught by validation
                    FormatOption::Type if field.kind.supports_name_type_options() => {
                        unreachable!(
                            "field '{}' has only :type occurrence - this should have been caught by validation",
                            field
                        )
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

                match (name_occ, type_occ) {
                    // SSA/Result with both :name and :type
                    (Some(name), Some(ty)) => field
                        .kind
                        .construct_from_name_and_type(crate_path, &name.var_name, &ty.var_name)
                        .unwrap_or_else(|| {
                            let var = &occs[0].var_name;
                            quote! { #var }
                        }),
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
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        _ast_generics: &syn::Generics,
        variant: Option<&syn::Ident>,
        wrapper: &kirin_derive_core::ir::fields::Wrapper,
        crate_path: &syn::Path,
        _dialect_type: &TokenStream,
    ) -> syn::Result<TokenStream> {
        let wrapped_ty = &wrapper.ty;

        let constructor = match variant {
            Some(v) => quote! { #ast_name::#v },
            None => quote! { #ast_name },
        };

        // Use explicit return type annotation to pin the lifetimes correctly
        // Use generic Language since this is inside HasDialectParser::recursive_parser.
        let return_type = self.build_ast_type_reference_generic(ir_input, ast_name);
        Ok(quote! {
            <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src, Language>>::recursive_parser(language.clone())
                .map(|inner| -> #return_type { #constructor(inner) })
        })
    }

    fn token_parser(&self, tokens: &[kirin_lexer::Token<'_>]) -> TokenStream {
        let crate_path = &self.config.crate_path;
        let mut iter = tokens.iter();
        let Some(first) = iter.next() else {
            return quote! { #crate_path::chumsky::prelude::empty().ignored() };
        };
        let mut parser = quote! { #crate_path::chumsky::prelude::just(#first) };
        for tok in iter {
            parser = quote! { #parser.then_ignore(#crate_path::chumsky::prelude::just(#tok)) };
        }
        parser
    }
}

enum ParserPart {
    Token(TokenStream),
    Field(TokenStream),
}
