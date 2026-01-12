use darling::{FromDeriveInput, FromVariant, FromField};

#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(chumsky))]
pub struct ChumskyGlobalAttrs {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, FromVariant, FromDeriveInput)]
#[darling(attributes(chumsky))]
pub struct ChumskyStatementAttrs {
    pub format: Option<String>,
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
}

#[derive(Debug, Clone, FromField)]
#[darling(attributes(chumsky))]
pub struct ChumskyFieldAttrs {}
