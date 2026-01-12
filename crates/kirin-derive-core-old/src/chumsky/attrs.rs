use crate::prelude::*;
use darling::{FromDeriveInput, FromVariant};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(chumsky))]
pub struct ChumskyEnumOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(chumsky))]
pub struct ChumskyStructOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    pub format: Option<String>,
}

#[derive(Debug, FromVariant)]
#[darling(attributes(chumsky))]
pub struct ChumskyVariantOptions {
    pub format: Option<String>,
}

impl WithUserCratePath for ChumskyEnumOptions {
    fn user_crate_path(&self) -> Option<&syn::Path> {
        self.crate_path.as_ref()
    }
}

impl WithUserCratePath for ChumskyStructOptions {
    fn user_crate_path(&self) -> Option<&syn::Path> {
        self.crate_path.as_ref()
    }
}
