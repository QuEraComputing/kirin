//! Code generation for chumsky derive macros.

mod ast;
mod bounds;
mod emit_ir;
mod parser;
mod pretty_print;

#[cfg(test)]
mod tests;

pub(crate) use bounds::BoundsBuilder;

use std::collections::HashSet;

use kirin_derive_core::ir::fields::FieldInfo;
use kirin_derive_core::ir::{Layout, VariantRef};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::ChumskyStatementAttrs;
use crate::field_kind::{ValueTypeScanner, collect_fields, fields_in_format};
use crate::format::Format;

pub use self::ast::GenerateAST;
pub use self::emit_ir::GenerateEmitIR;
pub use self::parser::GenerateHasDialectParser;
pub use self::pretty_print::GeneratePrettyPrint;

/// Shared configuration for code generators.
///
/// This extracts common paths from the IR input that all generators need.
#[derive(Clone)]
pub(crate) struct GeneratorConfig {
    /// Path to the kirin-chumsky crate (e.g., `::kirin::parsers`)
    pub crate_path: syn::Path,
    /// Path to the kirin IR crate (e.g., `::kirin::ir`)
    pub ir_path: syn::Path,
    /// The IR type path (e.g., `SimpleType`)
    pub ir_type: syn::Path,
}

impl GeneratorConfig {
    /// Creates a new generator config from IR input.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        let crate_path = ir_input
            .extra_attrs
            .crate_path
            .clone()
            .or(ir_input.attrs.crate_path.clone())
            .unwrap_or_else(|| syn::parse_quote!(::kirin::parsers));
        let ir_path = ir_input
            .attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| syn::parse_quote!(::kirin::ir));
        let ir_type = ir_input.attrs.ir_type.clone();
        Self {
            crate_path,
            ir_path,
            ir_type,
        }
    }
}

/// Builds AST generics shared by AST and EmitIR generators.
///
/// Returns generics like:
/// - without Language: `<'tokens, 'src, [original type params], TypeOutput, LanguageOutput>`
/// - with Language: `<'tokens, 'src, [original type params], TypeOutput, LanguageOutput, Language>`
pub(crate) fn build_ast_generics(
    base_generics: &syn::Generics,
    include_language: bool,
) -> syn::Generics {
    use proc_macro2::Span;

    let mut generics = base_generics.clone();

    let tokens_lt = syn::Lifetime::new("'tokens", Span::call_site());
    if !generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"))
    {
        generics.params.insert(
            0,
            syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt.clone())),
        );
    }

    let src_lt = syn::Lifetime::new("'src", Span::call_site());
    if !generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "src"))
    {
        let mut src_param = syn::LifetimeParam::new(src_lt);
        src_param.bounds.push(tokens_lt);
        generics
            .params
            .insert(1, syn::GenericParam::Lifetime(src_param));
    }

    let type_output_ident = syn::Ident::new("TypeOutput", Span::call_site());
    if !generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == type_output_ident))
    {
        generics
            .params
            .push(syn::GenericParam::Type(syn::TypeParam::from(
                type_output_ident,
            )));
    }

    let lang_output_ident = syn::Ident::new("LanguageOutput", Span::call_site());
    if !generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_output_ident))
    {
        generics
            .params
            .push(syn::GenericParam::Type(syn::TypeParam::from(
                lang_output_ident,
            )));
    }

    if include_language {
        let language_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == language_ident))
        {
            generics
                .params
                .push(syn::GenericParam::Type(syn::TypeParam::from(
                    language_ident,
                )));
        }
    }

    generics
}

/// Gets the format string for a statement from a layout that uses `ChumskyStatementAttrs`.
///
/// Checks statement-level `#[chumsky(format = ...)]`, then `#[kirin(format = ...)]`,
/// then falls back to a global format provider.
pub(crate) fn format_for_statement<L>(
    ir_input: &kirin_derive_core::ir::Input<L>,
    stmt: &kirin_derive_core::ir::Statement<L>,
) -> Option<String>
where
    L: Layout<ExtraStatementAttrs = ChumskyStatementAttrs>,
    L::ExtraGlobalAttrs: HasGlobalFormat,
{
    stmt.extra_attrs
        .format
        .clone()
        .or(stmt.attrs.format.clone())
        .or(ir_input.extra_attrs.global_format())
}

/// Trait for global attrs that may provide a fallback format string.
pub(crate) trait HasGlobalFormat {
    fn global_format(&self) -> Option<String>;
}

impl HasGlobalFormat for crate::ChumskyGlobalAttrs {
    fn global_format(&self) -> Option<String> {
        self.format.clone()
    }
}

impl HasGlobalFormat for crate::PrettyGlobalAttrs {
    fn global_format(&self) -> Option<String> {
        None
    }
}

/// Gets the set of field indices that are in the format string.
///
/// If there's no format string (e.g., wrapper variants), includes all fields.
pub(crate) fn get_fields_in_format(
    ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
) -> HashSet<usize> {
    let Some(format_str) = format_for_statement(ir_input, stmt) else {
        return collect_fields(stmt).iter().map(|f| f.index).collect();
    };

    match Format::parse(&format_str, None) {
        Ok(format) => fields_in_format(&format, stmt),
        Err(_) => collect_fields(stmt).iter().map(|f| f.index).collect(),
    }
}

/// Collects all Value field types that contain type parameters from all statements.
///
/// Uses the `Scan` visitor pattern from `kirin-derive-core` to traverse the IR.
/// These types need trait bounds (e.g., `HasParser`, `EmitIR`, `PrettyPrint`) in generated impls.
pub(crate) fn collect_all_value_types_needing_bounds(
    ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
) -> Vec<syn::Type> {
    ValueTypeScanner::new(&ir_input.generics)
        .scan(ir_input)
        .unwrap_or_default()
}

/// Collects all wrapper types from structs and enum variants.
///
/// Wrapper types need `HasDialectParser` bounds because we call
/// `HasDialectParser::recursive_parser(language)` on them to forward the Language parameter.
/// This is used to add the necessary trait bounds in generated impls.
pub(crate) fn collect_wrapper_types(
    ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
) -> Vec<syn::Type> {
    match &ir_input.data {
        kirin_derive_core::ir::Data::Struct(data) => {
            // For wrapper structs, return the wrapped type
            data.0
                .wraps
                .as_ref()
                .map(|w| vec![w.ty.clone()])
                .unwrap_or_default()
        }
        kirin_derive_core::ir::Data::Enum(data) => data
            .iter_variants()
            .filter_map(|variant| {
                if let VariantRef::Wrapper { wrapper, .. } = variant {
                    Some(wrapper.ty.clone())
                } else {
                    None
                }
            })
            .collect(),
    }
}

/// Filters collected fields to only include those needed in the AST.
///
/// Fields are included if they:
/// - Are in the format string (will be parsed), OR
/// - Don't have a default value (required)
///
/// This is used by both AST generation and EmitIR generation.
pub(crate) fn filter_ast_fields<'a>(
    collected: &'a [FieldInfo<ChumskyLayout>],
    fields_in_format: &HashSet<usize>,
) -> Vec<&'a FieldInfo<ChumskyLayout>> {
    collected
        .iter()
        .filter(|f| fields_in_format.contains(&f.index) || !f.has_default())
        .collect()
}

// =============================================================================
// Variant Iteration Helpers
// =============================================================================

/// Generates match arms for an enum, handling both wrapper and regular variants.
///
/// Uses `DataEnum::iter_variants()` from `kirin-derive-core` for variant classification.
///
/// - `type_name`: The enum type name (used in patterns like `TypeName::Variant`)
/// - `data`: The enum data containing variants
/// - `wrapper_handler`: Closure that generates code for wrapper variants
/// - `regular_handler`: Closure that generates code for regular variants
/// - `marker_handler`: Optional closure for the `__Marker` variant (for AST enums)
pub(crate) fn generate_enum_match<L: Layout, F, G>(
    type_name: &syn::Ident,
    data: &kirin_derive_core::ir::DataEnum<L>,
    wrapper_handler: F,
    regular_handler: G,
    marker_handler: Option<TokenStream>,
) -> TokenStream
where
    F: Fn(&syn::Ident, &kirin_derive_core::ir::fields::Wrapper) -> TokenStream,
    G: Fn(&syn::Ident, &kirin_derive_core::ir::Statement<L>) -> TokenStream,
{
    let arms: Vec<TokenStream> = data
        .iter_variants()
        .map(|variant| match variant {
            VariantRef::Wrapper { name, wrapper, .. } => {
                let body = wrapper_handler(name, wrapper);
                quote! { #type_name::#name(inner) => { #body } }
            }
            VariantRef::Regular { name, stmt } => regular_handler(name, stmt),
        })
        .collect();

    if let Some(marker) = marker_handler {
        quote! {
            match self {
                #(#arms)*
                #marker
            }
        }
    } else {
        quote! {
            match self {
                #(#arms)*
            }
        }
    }
}
