use crate::{
    attrs::{ChumskyEnumOptions, ChumskyStructOptions, ChumskyVariantOptions},
    parse::{Format, FormatOption},
};

use kirin_derive_core::{
    kirin::extra::{FieldKind, FieldMeta},
    prelude::*,
};
use syn::spanned::Spanned;

#[derive(Debug)]
pub struct SyntaxField {
    pub name: syn::Ident,
    pub crate_path: syn::Path,
    pub ty: TokenStream,
    pub kind: SyntaxFieldKind,
}

#[derive(Debug)]
pub enum SyntaxFieldKind {
    SSAValue,
    NameofSSAValue,
    TypeofSSAValue,
    TypeofResultValue,
    Block,
    Successor,
    Region,
    AsIs,
}

impl ToTokens for SyntaxField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let src_ty = &self.ty;
        let crate_path = &self.crate_path;
        let ty = match &self.kind {
            SyntaxFieldKind::SSAValue => {
                quote! { #crate_path::SSAValue<'tokens, 'src, Language> }
            }
            SyntaxFieldKind::NameofSSAValue => {
                quote! { #crate_path::NameofSSAValue<'src> }
            }
            SyntaxFieldKind::TypeofSSAValue => {
                quote! { #crate_path::TypeofSSAValue<'tokens, 'src, Language> }
            }
            SyntaxFieldKind::TypeofResultValue => {
                quote! { #crate_path::TypeofSSAValue<'tokens, 'src, Language> }
            }
            SyntaxFieldKind::Successor => {
                quote! { #crate_path::Successor<'tokens, 'src, Language> }
            }
            SyntaxFieldKind::Block => {
                quote! { #crate_path::Block<'tokens, 'src, Language> }
            }
            SyntaxFieldKind::Region => {
                quote! { #crate_path::Region<'tokens, 'src, Language> }
            }
            SyntaxFieldKind::AsIs => {
                quote! { <#src_ty as #crate_path::WithChumskyParser<'tokens, 'src, Language>>::Output }
            }
        };
        tokens.extend(quote! {
            pub #name: #ty,
        });
    }
}

pub struct DeriveAST {
    crate_path: syn::Path,
}

impl Layout for DeriveAST {
    type EnumAttr = ChumskyEnumOptions;
    type StructAttr = ChumskyStructOptions;
    type VariantAttr = ChumskyVariantOptions;
    type StatementExtra = Vec<SyntaxField>;
    type FieldAttr = ();
    type FieldExtra = FieldMeta;
}

impl<'src> ScanExtra<'src, syn::DeriveInput, Vec<SyntaxField>> for DeriveAST {
    fn scan_extra(&self, node: &'src syn::DeriveInput) -> syn::Result<Vec<SyntaxField>> {
        let syn::Data::Struct(ref s) = node.data else {
            return Err(syn::Error::new(
                node.ident.span(),
                "Chumsky derive only supports structs and enums",
            ));
        };
    }
}

impl<'src> ScanExtra<'src, Struct<'src, DeriveAST>, Vec<SyntaxField>> for DeriveAST {
    fn scan_extra(&self, node: &'src Struct<'src, DeriveAST>) -> syn::Result<Vec<SyntaxField>> {
        let Some(ref format) = node.attrs().format else {
            return Err(syn::Error::new(
                node.source_ident().span(),
                "Missing 'format' attribute on struct",
            ));
        };
        scan_fields(format, self, node.fields())
    }
}

impl<'src> ScanExtra<'src, Variant<'_, 'src, DeriveAST>, Vec<SyntaxField>> for DeriveAST {
    fn scan_extra(
        &self,
        node: &'src Variant<'_, 'src, DeriveAST>,
    ) -> syn::Result<Vec<SyntaxField>> {
        let Some(ref format) = node.attrs().format else {
            return Err(syn::Error::new(
                node.source_ident().span(),
                "Missing 'format' attribute on struct",
            ));
        };
        scan_fields(format, self, node.fields())
    }
}

fn scan_fields<'src>(
    format: &String,
    ctx: &DeriveAST,
    node: Fields<'_, 'src, DeriveAST>,
) -> syn::Result<Vec<SyntaxField>> {
    let format = Format::parse(&format, None)?;
    // ResultValue field will not appear in the generated AST node, because the upper-level AST
    // will always hold the ResultValue directly as syntax `<result> = <statement>`
    let mut fs = Vec::new();
    let mut err = Vec::new();
    for f in node.iter() {
        let s = f.source_ident().to_string();
        let kind = match (format.get_field(&s), &f.extra().kind) {
            (Some(FormatOption::Default), FieldKind::SSAValue) => SyntaxFieldKind::SSAValue,
            (Some(FormatOption::Default), FieldKind::ResultValue) => {
                err.push(syn::Error::new(
                    f.source().span(),
                    "cannot specify ResultValue field in the format text",
                ));
                continue;
            }
            (Some(FormatOption::Default), FieldKind::Block) => SyntaxFieldKind::Block,
            (Some(FormatOption::Default), FieldKind::Successor) => SyntaxFieldKind::Successor,
            (Some(FormatOption::Default), FieldKind::Region) => SyntaxFieldKind::Region,
            (Some(FormatOption::Name), FieldKind::SSAValue) => SyntaxFieldKind::NameofSSAValue,
            (Some(FormatOption::Name), FieldKind::ResultValue) => {
                err.push(syn::Error::new(
                    f.source().span(),
                    "cannot specify ResultValue field name in the format text",
                ));
                continue;
            }
            // {block} is the syntax sugar for {block:name}
            (Some(FormatOption::Name), FieldKind::Block) => SyntaxFieldKind::Block,
            (Some(FormatOption::Name), FieldKind::Successor) => SyntaxFieldKind::Successor,
            (Some(FormatOption::Name), _) => {
                err.push(syn::Error::new(
                    f.source().span(),
                    "only SSAValue, Block, and Successor fields can use 'name' format option",
                ));
                continue;
            }

            (Some(FormatOption::Type), FieldKind::SSAValue) => SyntaxFieldKind::TypeofSSAValue,
            (Some(FormatOption::Type), FieldKind::ResultValue) => {
                SyntaxFieldKind::TypeofResultValue
            }
            (Some(FormatOption::Type), _) => {
                err.push(syn::Error::new(
                    f.source().span(),
                    "only SSAValue and ResultValue fields can use 'type' format option",
                ));
                continue;
            }
            _ => SyntaxFieldKind::AsIs,
        };

        fs.push(SyntaxField {
            name: f.source_ident(),
            crate_path: node
                .user_crate_path()
                .cloned()
                .unwrap_or_else(|| ctx.crate_path.clone()),
            ty: f.source().ty.to_token_stream(),
            kind,
        });
    }

    if !err.is_empty() {
        let combined = err
            .into_iter()
            .reduce(|mut acc, e| {
                acc.combine(e);
                acc
            })
            .unwrap();
        return Err(combined);
    }
    Ok(fs)
}

target! {
    pub struct StructImpl;
}

impl<'src> Compile<'src, DeriveAST, StructImpl> for Struct<'src, DeriveAST> {
    fn compile(&self, _ctx: &DeriveAST) -> StructImpl {
        let struct_name = format_ident!("AbstractSyntaxTree{}", self.source_ident());
        let fields = self.extra();
        quote! {
            pub struct #struct_name {
                #(#fields)*
            }
        }
        .into()
    }
}

target! {
    pub struct EnumImpl;
}

impl<'src> Compile<'src, DeriveAST, EnumImpl> for Enum<'src, DeriveAST> {
    fn compile(&self, _ctx: &DeriveAST) -> EnumImpl {
        let enum_name = format_ident!("AbstractSyntaxTree{}", self.source_ident());
        let variants = self.variants().map(|v| {
            let var_name = format_ident!("{}", v.source_ident());
            let var_fields = v.extra();
            quote! {
                #var_name {
                    #(#var_fields)*
                }
            }
        });
        quote! {
            pub enum #enum_name {
                #(#variants),*
            }
        }
        .into()
    }
}

impl<'src> Emit<'src> for DeriveAST {
    type EnumImpl = EnumImpl;
    type StructImpl = StructImpl;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_derive_core::prelude::*;

    #[test]
    fn test_struct_basic() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[chumsky(format = "{name} = add {lhs}, {rhs}")]
            pub struct Add {
                pub name: SSAValue,
                pub lhs: SSAValue,
                pub rhs: SSAValue,
            }
        };

        let derive_ctx = DeriveAST {
            crate_path: syn::parse_quote! { kirin_chumsky },
        };

        derive_ctx.print(&input).unwrap();
    }
}