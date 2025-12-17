use crate::data::AttrCratePath;
use darling::{Error, FromDeriveInput, FromField, FromMeta, FromVariant, util::Ignored};

#[derive(Debug, FromField)]
#[darling(attributes(kirin))]
pub struct KirinFieldOptions {
    #[darling(default)]
    pub into: bool,
    pub default: Option<syn::Expr>,
    #[darling(rename = "type")]
    pub ssa_ty: Option<syn::Type>,
}

#[derive(Debug, FromVariant, FromDeriveInput)]
#[darling(attributes(kirin))]
pub struct KirinStatementOptions {
    pub format: Option<String>,
    #[darling(rename = "fn")]
    pub builder: Option<Builder>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub terminator: bool,
    // note that global options are accepted by structs thus ignored here
    #[darling(default, rename = "crate")]
    pub crate_path: Ignored,
    #[darling(default)]
    pub type_lattice: Ignored,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(kirin))]
pub struct KirinGlobalOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    pub type_lattice: syn::Path,
    #[darling(rename = "fn")]
    pub builder: Option<Builder>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub terminator: bool,
}

impl AttrCratePath for KirinGlobalOptions {
    fn crate_path(&self) -> Option<&syn::Path> {
        self.crate_path.as_ref()
    }
}

#[derive(Debug)]
pub enum Builder {
    Enabled,
    Named(syn::Ident),
}

impl FromMeta for Builder {
    fn from_word() -> darling::Result<Self> {
        Ok(Builder::Enabled)
    }

    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        match expr {
            syn::Expr::Path(syn::ExprPath { path, .. }) => {
                if let Some(ident) = path.get_ident() {
                    Ok(Builder::Named(ident.clone()))
                } else {
                    Err(Error::custom("Expected identifier for builder name"))
                }
            }
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => {
                let ident = syn::Ident::new(&s.value(), s.span());
                Ok(Builder::Named(ident))
            }
            _ => Err(Error::custom(
                "Expected identifier or string for builder name",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_global_options() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(crate = some::path, type_lattice = MyLattice, fn = start_function)]
            struct MyStruct {
                field1: i32,
            }
        };

        let options = KirinStatementOptions::from_derive_input(&input).unwrap();
        println!("{:?}", options);
    }
}
