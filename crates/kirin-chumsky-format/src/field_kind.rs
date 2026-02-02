//! Field kind enumeration for code generation.
//!
//! This module provides a unified `FieldKind` type used by both AST generation
//! and parser generation.

use std::collections::HashSet;

use kirin_derive_core::ir::{fields::Collection, DefaultValue};
use kirin_derive_core::misc::is_type_in_generic;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::format::{Format, FormatElement, FormatOption};

/// The kind of a field in code generation context.
///
/// This extends `FieldCategory` with the actual type information for value fields.
#[derive(Debug, Clone)]
pub enum FieldKind {
    /// SSAValue input field
    SSAValue,
    /// ResultValue output field
    ResultValue,
    /// Block field (owned control flow block)
    Block,
    /// Successor field (branch target)
    Successor,
    /// Region field (nested scope)
    Region,
    /// Compile-time value field with its type
    Value(syn::Type),
}

impl FieldKind {
    /// Returns a human-readable name for this field kind.
    pub fn name(&self) -> &'static str {
        match self {
            FieldKind::SSAValue => "ssa_value",
            FieldKind::ResultValue => "result_value",
            FieldKind::Block => "block",
            FieldKind::Successor => "successor",
            FieldKind::Region => "region",
            FieldKind::Value(_) => "value",
        }
    }

    /// Returns true if this field kind supports the :name and :type format options.
    pub fn supports_name_type_options(&self) -> bool {
        matches!(self, FieldKind::SSAValue | FieldKind::ResultValue)
    }

    /// Generates the AST type for this field kind.
    ///
    /// The `crate_path` should be the path to the kirin_chumsky crate.
    /// The `ast_name` is the name of the AST type (e.g., `TestLangAST`) used for Block/Region statements.
    /// The `type_lattice` is the type lattice path (e.g., `SimpleType`) used for type annotations.
    pub fn ast_type(
        &self,
        crate_path: &syn::Path,
        ast_name: &syn::Ident,
        type_lattice: &syn::Path,
    ) -> TokenStream {
        // Use <type_lattice as HasParser>::Output for type annotations.
        // This matches the TypeAST definition in HasDialectParser impl.
        // For Block/Region, use the concrete AST type to avoid circular trait bounds.
        let type_ast = quote! { <#type_lattice as #crate_path::HasParser<'tokens, 'src>>::Output };
        let stmt_output = quote! { #ast_name<'tokens, 'src, Language> };

        match self {
            FieldKind::SSAValue => {
                quote! { #crate_path::SSAValue<'src, #type_ast> }
            }
            FieldKind::ResultValue => {
                quote! { #crate_path::ResultValue<'src, #type_ast> }
            }
            FieldKind::Block => {
                // Block parser returns Spanned<Block>, so we need Spanned wrapper
                quote! { #crate_path::Spanned<#crate_path::Block<'src, #type_ast, #stmt_output>> }
            }
            FieldKind::Successor => {
                quote! { #crate_path::BlockLabel<'src> }
            }
            FieldKind::Region => {
                quote! { #crate_path::Region<'src, #type_ast, #stmt_output> }
            }
            FieldKind::Value(ty) => {
                quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::Output }
            }
        }
    }

    /// Generates the parser expression for this field kind.
    ///
    /// For SSAValue/ResultValue fields, the `opt` parameter controls which parser to use:
    /// - `Default`: full value parser with optional type annotation
    /// - `Name`: name-only parser
    /// - `Type`: type-only parser
    ///
    /// The `crate_path` should be the path to the kirin_chumsky crate.
    /// The `_dialect_type` is unused but kept for API compatibility.
    /// The `ast_name` should be the AST type name (e.g., `TestLangAST`) for Block/Region field transmutation.
    /// The `type_lattice` should be the concrete type lattice (e.g., `SimpleType`) used for type annotations.
    pub fn parser_expr(
        &self,
        crate_path: &syn::Path,
        opt: &FormatOption,
        _dialect_type: &TokenStream,
        ast_name: &syn::Ident,
        type_lattice: &syn::Path,
    ) -> TokenStream {
        match self {
            FieldKind::SSAValue => match opt {
                FormatOption::Name => quote! { #crate_path::nameof_ssa() },
                FormatOption::Type => {
                    quote! { #crate_path::typeof_ssa::<_, Language, #type_lattice>() }
                }
                FormatOption::Default => {
                    quote! { #crate_path::ssa_value::<_, Language, #type_lattice>() }
                }
            },
            FieldKind::ResultValue => match opt {
                FormatOption::Name => quote! { #crate_path::nameof_ssa() },
                FormatOption::Type => {
                    quote! { #crate_path::typeof_ssa::<_, Language, #type_lattice>() }
                }
                FormatOption::Default => {
                    quote! { #crate_path::result_value_with_optional_type::<_, Language, #type_lattice>() }
                }
            },
            FieldKind::Block => {
                // Block parser uses Language as the language parameter.
                // Parser returns Block<..., <Language as HasDialectParser>::Output>
                // AST type is Block<..., AST<..., Language>>
                // These are the same type when Language: HasDialectParser, so the transmute is safe.
                let type_ast = quote! { <#type_lattice as #crate_path::HasParser<'tokens, 'src>>::Output };
                quote! {
                    #crate_path::block::<_, Language, #type_lattice>(language.clone())
                        .map(|b| unsafe {
                            ::core::mem::transmute::<
                                #crate_path::Spanned<#crate_path::Block<'src, #type_ast, <Language as #crate_path::HasDialectParser<'tokens, 'src, Language>>::Output>>,
                                #crate_path::Spanned<#crate_path::Block<'src, #type_ast, #ast_name<'tokens, 'src, Language>>>
                            >(b)
                        })
                }
            }
            FieldKind::Successor => {
                quote! { #crate_path::block_label() }
            }
            FieldKind::Region => {
                // Region parser uses Language as the language parameter.
                // Parser returns Region<..., <Language as HasDialectParser>::Output>
                // AST type is Region<..., AST<..., Language>>
                // These are the same type when Language: HasDialectParser, so the transmute is safe.
                let type_ast = quote! { <#type_lattice as #crate_path::HasParser<'tokens, 'src>>::Output };
                quote! {
                    #crate_path::region::<_, Language, #type_lattice>(language.clone())
                        .map(|r| unsafe {
                            ::core::mem::transmute::<
                                #crate_path::Region<'src, #type_ast, <Language as #crate_path::HasDialectParser<'tokens, 'src, Language>>::Output>,
                                #crate_path::Region<'src, #type_ast, #ast_name<'tokens, 'src, Language>>
                            >(r)
                        })
                }
            }
            FieldKind::Value(ty) => {
                quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::parser() }
            }
        }
    }

    /// Returns the AST type name for SSA-like fields (SSAValue or ResultValue).
    ///
    /// Returns None for non-SSA field kinds.
    fn ssa_type_name(&self) -> Option<&'static str> {
        match self {
            FieldKind::SSAValue => Some("SSAValue"),
            FieldKind::ResultValue => Some("ResultValue"),
            _ => None,
        }
    }

    /// Generates constructor code when only the :name format option is provided.
    ///
    /// This creates an SSA/Result value with `ty: None`.
    /// Returns None for non-SSA field kinds.
    pub fn construct_from_name_only(
        &self,
        crate_path: &syn::Path,
        name_var: &syn::Ident,
    ) -> Option<TokenStream> {
        let type_name = syn::Ident::new(self.ssa_type_name()?, proc_macro2::Span::call_site());
        Some(quote! {
            #crate_path::#type_name {
                name: #crate_path::Spanned { value: #name_var.name, span: #name_var.span },
                ty: None,
            }
        })
    }

    /// Generates constructor code when both :name and :type format options are provided.
    ///
    /// This creates an SSA/Result value with both name and type fields populated.
    /// Returns None for non-SSA field kinds.
    pub fn construct_from_name_and_type(
        &self,
        crate_path: &syn::Path,
        name_var: &syn::Ident,
        type_var: &syn::Ident,
    ) -> Option<TokenStream> {
        let type_name = syn::Ident::new(self.ssa_type_name()?, proc_macro2::Span::call_site());
        Some(quote! {
            #crate_path::#type_name {
                name: #crate_path::Spanned { value: #name_var.name, span: #name_var.span },
                ty: Some(#type_var.ty.clone()),
            }
        })
    }

    /// Generates pretty print expression for this field kind.
    ///
    /// For SSAValue/ResultValue fields, the `opt` parameter controls which printer to use:
    /// - `Default`: full value printer (name + optional type)
    /// - `Name`: name-only printer
    /// - `Type`: type-only printer
    ///
    /// The `prettyless_path` should be the path to the kirin_prettyless crate.
    /// Note: `field_ref` should be a variable that is already a reference (from pattern matching).
    pub fn print_expr(
        &self,
        prettyless_path: &syn::Path,
        field_ref: &TokenStream,
        opt: &FormatOption,
    ) -> TokenStream {
        match self {
            FieldKind::SSAValue | FieldKind::ResultValue => match opt {
                FormatOption::Name => quote! {
                    #prettyless_path::PrettyPrintName::pretty_print_name(#field_ref, doc)
                },
                FormatOption::Type => quote! {
                    #prettyless_path::PrettyPrintType::pretty_print_type(#field_ref, doc)
                },
                FormatOption::Default => quote! {
                    #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
                },
            },
            FieldKind::Block | FieldKind::Successor | FieldKind::Region => quote! {
                #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
            },
            FieldKind::Value(_ty) => {
                // For compile-time values, use Display trait
                quote! {
                    doc.text(format!("{}", #field_ref))
                }
            }
        }
    }
}

/// Collected field information used during code generation.
///
/// This combines the field index, identifier, collection type, and kind
/// into a single structure for processing.
#[derive(Debug, Clone)]
pub struct CollectedField {
    /// The positional index of this field
    pub index: usize,
    /// The field identifier (None for tuple fields)
    pub ident: Option<syn::Ident>,
    /// The collection type (Single, Vec, Option)
    pub collection: Collection,
    /// The kind of this field
    pub kind: FieldKind,
    /// The default value if specified via `#[kirin(default)]` or `#[kirin(default = ...)]`
    pub default: Option<DefaultValue>,
}

impl std::fmt::Display for CollectedField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.ident {
            Some(ident) => write!(f, "{}", ident),
            None => write!(f, "field_{}", self.index),
        }
    }
}

/// Collects all fields from a statement.
///
/// Fields are returned in the same order as `Statement::iter_all_fields()`:
/// arguments, results, blocks, successors, regions, values.
/// This ensures consistency with `Statement::field_bindings()`.
pub fn collect_fields(
    stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
) -> Vec<CollectedField> {
    let mut fields = Vec::new();

    for arg in stmt.arguments.iter() {
        fields.push(CollectedField {
            index: arg.field.index,
            ident: arg.field.ident.clone(),
            collection: arg.collection.clone(),
            kind: FieldKind::SSAValue,
            default: None, // SSAValue fields don't support defaults
        });
    }

    for res in stmt.results.iter() {
        fields.push(CollectedField {
            index: res.field.index,
            ident: res.field.ident.clone(),
            collection: res.collection.clone(),
            kind: FieldKind::ResultValue,
            default: None, // ResultValue fields don't support defaults
        });
    }

    for block in stmt.blocks.iter() {
        fields.push(CollectedField {
            index: block.field.index,
            ident: block.field.ident.clone(),
            collection: block.collection.clone(),
            kind: FieldKind::Block,
            default: None, // Block fields don't support defaults
        });
    }

    for succ in stmt.successors.iter() {
        fields.push(CollectedField {
            index: succ.field.index,
            ident: succ.field.ident.clone(),
            collection: succ.collection.clone(),
            kind: FieldKind::Successor,
            default: None, // Successor fields don't support defaults
        });
    }

    for region in stmt.regions.iter() {
        fields.push(CollectedField {
            index: region.field.index,
            ident: region.field.ident.clone(),
            collection: region.collection.clone(),
            kind: FieldKind::Region,
            default: None, // Region fields don't support defaults
        });
    }

    for value in stmt.values.iter() {
        fields.push(CollectedField {
            index: value.field.index,
            ident: value.field.ident.clone(),
            collection: Collection::Single,
            kind: FieldKind::Value(value.ty.clone()),
            default: value.default.clone(), // Compile-time values can have defaults
        });
    }

    // NOTE: Do NOT sort here! The order must match Statement::iter_all_fields()
    // which is used by Statement::field_bindings() for code generation.
    fields
}

/// Collects Value field types that contain the given type parameters.
///
/// For example, if a struct has `T: Clone` and a field `value: T`,
/// this will return `vec![T]` (the type that needs HasParser bounds).
///
/// This is used to generate appropriate where clauses for generic types.
/// Only includes fields that don't have a default value, since those are
/// the only ones that need to be parsed.
pub fn collect_value_types_with_type_params(
    collected: &[CollectedField],
    generics: &syn::Generics,
) -> Vec<syn::Type> {
    // Get type parameter names from the generics
    let type_param_names: Vec<_> = generics
        .params
        .iter()
        .filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    if type_param_names.is_empty() {
        return Vec::new();
    }

    let mut types_needing_bounds = Vec::new();

    for field in collected {
        // Only consider fields without defaults - fields with defaults are not parsed
        if field.default.is_some() {
            continue;
        }

        if let FieldKind::Value(ty) = &field.kind {
            // Check if any type parameter appears in this field's type
            for param_name in &type_param_names {
                // Check if the type IS the parameter directly
                if kirin_derive_core::misc::is_type(ty, param_name.as_str()) {
                    types_needing_bounds.push(ty.clone());
                    break;
                }
                // Check if the type parameter appears inside a generic type
                if is_type_in_generic(ty, param_name.as_str()) {
                    types_needing_bounds.push(ty.clone());
                    break;
                }
            }
        }
    }

    // Deduplicate
    let mut seen = HashSet::new();
    types_needing_bounds.retain(|ty| {
        let key = quote::quote!(#ty).to_string();
        seen.insert(key)
    });

    types_needing_bounds
}

/// Returns the set of field indices that are mentioned in the format string.
///
/// This is used to determine which fields need to be included in the AST
/// (fields not in format string but with defaults are excluded).
pub fn fields_in_format(
    format: &Format<'_>,
    stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
) -> HashSet<usize> {
    let map_by_ident = stmt.field_name_to_index();
    let mut indices = HashSet::new();

    for elem in format.elements() {
        if let FormatElement::Field(name, _) = elem {
            // Try to parse as index first, then look up by name
            let index = name
                .parse::<usize>()
                .ok()
                .or_else(|| map_by_ident.get(&name.to_string()).copied());
            if let Some(idx) = index {
                indices.insert(idx);
            }
        }
    }

    indices
}

