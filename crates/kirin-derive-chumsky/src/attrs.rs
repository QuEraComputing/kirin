//! Attribute definitions for chumsky derive macros.

use darling::{FromDeriveInput, FromField, FromVariant};
use kirin_derive_toolkit::ir::HasCratePath;

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

impl HasCratePath for ChumskyGlobalAttrs {
    fn crate_path(&self) -> Option<&syn::Path> {
        self.crate_path.as_ref()
    }
}

/// Attributes applied to individual statements or enum variants.
#[derive(Debug, Clone, FromVariant)]
#[darling(attributes(chumsky))]
pub struct ChumskyStatementAttrs {
    /// The format string for this statement.
    pub format: Option<String>,
}

/// Attributes applied to individual fields.
#[derive(Debug, Clone, Default, FromField)]
#[darling(attributes(chumsky))]
pub struct ChumskyFieldAttrs {
    // Currently no field-level chumsky attributes
}

/// Global attributes for the `PrettyPrint` derive macro.
///
/// Pretty printing intentionally reuses `#[chumsky(format = ...)]` for layout,
/// while `#[pretty(crate = ...)]` remains the crate-path override.
#[derive(Debug, Clone)]
pub struct PrettyGlobalAttrs {
    /// The path to the kirin-prettyless crate.
    pub crate_path: Option<syn::Path>,
    /// Default format string for all variants/statements, read from `#[chumsky(format = ...)]`.
    pub format: Option<String>,
}

impl FromDeriveInput for PrettyGlobalAttrs {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        #[derive(FromDeriveInput)]
        #[darling(attributes(pretty), allow_unknown_fields)]
        struct PrettyAttrs {
            #[darling(rename = "crate")]
            crate_path: Option<syn::Path>,
        }

        #[derive(FromDeriveInput)]
        #[darling(attributes(chumsky), allow_unknown_fields)]
        struct ChumskyAttrs {
            format: Option<String>,
        }

        let pretty = PrettyAttrs::from_derive_input(input)?;
        let chumsky = ChumskyAttrs::from_derive_input(input)?;
        Ok(Self {
            crate_path: pretty.crate_path,
            format: chumsky.format,
        })
    }
}

impl HasCratePath for PrettyGlobalAttrs {
    fn crate_path(&self) -> Option<&syn::Path> {
        self.crate_path.as_ref()
    }
}
