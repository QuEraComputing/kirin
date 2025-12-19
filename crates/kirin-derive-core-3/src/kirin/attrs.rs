use crate::prelude::*;
use darling::{Error, FromDeriveInput, FromField, FromMeta, FromVariant};

#[derive(Debug, FromField)]
#[darling(attributes(kirin))]
pub struct KirinFieldOptions {
    #[darling(default)]
    pub into: bool,
    pub default: Option<syn::Expr>,
    #[darling(rename = "type")]
    pub ssa_ty: Option<syn::Type>,
}
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(kirin))]
pub struct KirinEnumOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    pub type_lattice: syn::Path,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub terminator: bool,
}

impl WithUserCratePath for KirinEnumOptions {
    fn crate_path(&self) -> Option<&syn::Path> {
        self.crate_path.as_ref()
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(kirin))]
pub struct KirinStructOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    pub type_lattice: syn::Path,
    pub format: Option<String>,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub terminator: bool,
}

impl WithUserCratePath for KirinStructOptions {
    fn crate_path(&self) -> Option<&syn::Path> {
        self.crate_path.as_ref()
    }
}

#[derive(Debug, FromVariant)]
#[darling(attributes(kirin))]
pub struct KirinVariantOptions {
    pub format: Option<String>,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub terminator: bool,
}

#[derive(Debug, Clone)]
pub enum BuilderOptions {
    Enabled,
    Named(syn::Ident),
}

impl FromMeta for BuilderOptions {
    fn from_word() -> darling::Result<Self> {
        Ok(BuilderOptions::Enabled)
    }

    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        match expr {
            syn::Expr::Path(syn::ExprPath { path, .. }) => {
                if let Some(ident) = path.get_ident() {
                    Ok(BuilderOptions::Named(ident.clone()))
                } else {
                    Err(Error::custom("Expected identifier for builder name"))
                }
            }
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => {
                let ident = syn::Ident::new(&s.value(), s.span());
                Ok(BuilderOptions::Named(ident))
            }
            _ => Err(Error::custom(
                "Expected identifier or string for builder name",
            )),
        }
    }
}
