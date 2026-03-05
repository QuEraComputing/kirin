//! Field kind enumeration for code generation.

use kirin_derive_toolkit::ir::Layout;
use kirin_derive_toolkit::ir::fields::FieldCategory;
use kirin_derive_toolkit::ir::fields::FieldInfo;
use proc_macro2::TokenStream;
use quote::quote;

use crate::format::FormatOption;

/// The kind of a field in code generation context.
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
    /// Symbol field (global symbol reference)
    Symbol,
    /// Compile-time value field with its type
    Value(syn::Type),
}

impl FieldKind {
    /// Creates a `FieldKind` from a `FieldInfo`.
    pub fn from_field_info<L: Layout>(field: &FieldInfo<L>) -> Self {
        match field.category() {
            FieldCategory::Argument => FieldKind::SSAValue,
            FieldCategory::Result => FieldKind::ResultValue,
            FieldCategory::Block => FieldKind::Block,
            FieldCategory::Successor => FieldKind::Successor,
            FieldCategory::Region => FieldKind::Region,
            FieldCategory::Symbol => FieldKind::Symbol,
            FieldCategory::Value => {
                let ty = field
                    .value_type()
                    .cloned()
                    .unwrap_or_else(|| syn::parse_quote!(()));
                FieldKind::Value(ty)
            }
        }
    }

    /// Returns a human-readable name for this field kind.
    pub fn name(&self) -> &'static str {
        match self {
            FieldKind::SSAValue => "ssa_value",
            FieldKind::ResultValue => "result_value",
            FieldKind::Block => "block",
            FieldKind::Successor => "successor",
            FieldKind::Region => "region",
            FieldKind::Symbol => "symbol",
            FieldKind::Value(_) => "value",
        }
    }

    /// Returns true if this field kind supports the :name and :type format options.
    pub fn supports_name_type_options(&self) -> bool {
        matches!(self, FieldKind::SSAValue | FieldKind::ResultValue)
    }

    /// Generates the AST type for this field kind.
    pub fn ast_type(
        &self,
        crate_path: &syn::Path,
        _ast_name: &syn::Ident,
        ir_type: &syn::Path,
        _type_params: &[TokenStream],
    ) -> TokenStream {
        let type_output = quote! { <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output };
        match self {
            FieldKind::SSAValue => {
                quote! { #crate_path::SSAValue<'src, #type_output> }
            }
            FieldKind::ResultValue => {
                quote! { #crate_path::ResultValue<'src, #type_output> }
            }
            FieldKind::Block => {
                quote! { #crate_path::Spanned<#crate_path::Block<'src, #type_output, LanguageOutput>> }
            }
            FieldKind::Successor => {
                quote! { #crate_path::BlockLabel<'src> }
            }
            FieldKind::Region => {
                quote! { #crate_path::Region<'src, #type_output, LanguageOutput> }
            }
            FieldKind::Symbol => {
                quote! { #crate_path::SymbolName<'src> }
            }
            FieldKind::Value(ty) => {
                quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::Output }
            }
        }
    }

    /// Generates the parser expression for this field kind.
    pub fn parser_expr(
        &self,
        crate_path: &syn::Path,
        opt: &FormatOption,
        _ast_name: &syn::Ident,
        ir_type: &syn::Path,
        _type_params: &[TokenStream],
    ) -> TokenStream {
        match self {
            FieldKind::SSAValue => match opt {
                FormatOption::Name => quote! { #crate_path::nameof_ssa() },
                FormatOption::Type => {
                    quote! { #crate_path::typeof_ssa::<_, #ir_type>() }
                }
                FormatOption::Default => {
                    quote! { #crate_path::ssa_value::<_, #ir_type>() }
                }
            },
            FieldKind::ResultValue => match opt {
                FormatOption::Name => quote! { #crate_path::nameof_ssa() },
                FormatOption::Type => {
                    quote! { #crate_path::typeof_ssa::<_, #ir_type>() }
                }
                FormatOption::Default => {
                    quote! { #crate_path::result_value::<_, #ir_type>() }
                }
            },
            FieldKind::Block => {
                quote! {
                    #crate_path::block::<_, #ir_type, _>(language.clone())
                }
            }
            FieldKind::Successor => {
                quote! { #crate_path::block_label() }
            }
            FieldKind::Region => {
                quote! {
                    #crate_path::region::<_, #ir_type, _>(language.clone())
                }
            }
            FieldKind::Symbol => {
                quote! { #crate_path::symbol() }
            }
            FieldKind::Value(ty) => {
                quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::parser() }
            }
        }
    }

    /// Returns the AST type name for SSA-like fields (SSAValue or ResultValue).
    fn ssa_type_name(&self) -> Option<&'static str> {
        match self {
            FieldKind::SSAValue => Some("SSAValue"),
            FieldKind::ResultValue => Some("ResultValue"),
            _ => None,
        }
    }

    /// Generates constructor code when only the :name format option is provided.
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
    pub fn print_expr(
        &self,
        prettyless_path: &syn::Path,
        field_ref: &TokenStream,
        opt: &FormatOption,
    ) -> TokenStream {
        match self {
            FieldKind::SSAValue | FieldKind::ResultValue => match opt {
                FormatOption::Name => quote! {
                    #prettyless_path::PrettyPrint::pretty_print_name(#field_ref, doc)
                },
                FormatOption::Type => quote! {
                    #prettyless_path::PrettyPrint::pretty_print_type(#field_ref, doc)
                },
                FormatOption::Default => quote! {
                    #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
                },
            },
            FieldKind::Block => quote! {
                doc.print_block(#field_ref)
            },
            FieldKind::Successor => quote! {
                #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
            },
            FieldKind::Region => quote! {
                doc.print_region(#field_ref)
            },
            FieldKind::Symbol => quote! {
                #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
            },
            FieldKind::Value(_ty) => {
                quote! {
                    #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
                }
            }
        }
    }
}

/// Collects all fields from a statement using the method on Statement.
pub fn collect_fields<L: Layout>(stmt: &kirin_derive_toolkit::ir::Statement<L>) -> Vec<FieldInfo<L>> {
    stmt.collect_fields()
}
