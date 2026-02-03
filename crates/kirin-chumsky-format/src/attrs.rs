//! Attribute definitions for chumsky derive macros.

use darling::{FromDeriveInput, FromField, FromVariant};

/// Global attributes applied to the entire derive input.
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(chumsky))]
pub struct ChumskyGlobalAttrs {
    /// The path to the kirin-chumsky crate.
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,

    /// Default format string for all variants/statements.
    pub format: Option<String>,
}

/// Attributes applied to individual statements or enum variants.
#[derive(Debug, Clone, FromVariant, FromDeriveInput)]
#[darling(attributes(chumsky))]
pub struct ChumskyStatementAttrs {
    /// The format string for this statement.
    pub format: Option<String>,

    /// Override crate path for this statement.
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
}

/// Attributes applied to individual fields.
#[derive(Debug, Clone, Default, FromField)]
#[darling(attributes(chumsky))]
pub struct ChumskyFieldAttrs {
    // Currently no field-level chumsky attributes
}
