//! Parsed `#[kirin(...)]` attributes for dialect derive macros.
//!
//! This module handles the **`#[kirin(...)]`** carry attribute, which contains
//! dialect-specific options (type, crate path, format, builder, markers).
//!
//! Helper attributes like **`#[wraps]`** and **`#[callable]`** are intentionally
//! *not* part of `#[kirin(...)]`. They are bare attributes parsed separately so
//! that different derive macros can compose independently:
//!
//! - `#[wraps]` — generic delegation/wrapper pattern, used by `#[derive(Dialect)]`
//!   and potentially other derives.
//! - `#[callable]` — interpreter-specific, used by `#[derive(CallSemantics)]`.
//!
//! This separation lets a single type opt into multiple derive macros without
//! coupling their attribute namespaces.

use darling::{Error, FromDeriveInput, FromField, FromMeta, FromVariant};

/// Parsed `#[kirin(...)]` attributes on a struct-level statement definition.
///
/// # Attributes
///
/// - `type = <Path>` — (**required**) the IR type this statement produces
///   (e.g. `ArithType`, `SimpleIRType::Float`).
/// - `crate = <Path>` — override the default IR crate path (`::kirin::ir`).
/// - `format = "<string>"` — custom format string for pretty-printing.
/// - `fn` / `fn = <ident>` — generate a builder function (auto-named or with the given name).
/// - `constant` — mark as a constant-foldable operation.
/// - `pure` — mark as a pure (side-effect-free) operation.
/// - `speculatable` — mark as safe to speculatively execute.
/// - `terminator` — mark as a block terminator.
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(kirin), supports(struct_any))]
pub struct KirinStructOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    #[darling(rename = "type")]
    pub ir_type: syn::Path,
    pub format: Option<String>,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub speculatable: bool,
    #[darling(default)]
    pub terminator: bool,
}

/// Parsed `#[kirin(...)]` attributes on an enum-level statement definition.
///
/// Same attributes as [`KirinStructOptions`] except `format` is specified per-variant
/// via [`StatementOptions`].
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(kirin), supports(enum_any))]
pub struct KirinEnumOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    #[darling(rename = "type")]
    pub ir_type: syn::Path,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub speculatable: bool,
    #[darling(default)]
    pub terminator: bool,
}

/// Parsed `#[kirin(...)]` attributes on an enum variant (one variant = one statement kind).
///
/// # Attributes
///
/// - `format = "<string>"` — custom format string for pretty-printing this variant.
/// - `fn` / `fn = <ident>` — generate a builder function for this variant.
/// - `constant` — mark as a constant-foldable operation.
/// - `pure` — mark as a pure (side-effect-free) operation.
/// - `speculatable` — mark as safe to speculatively execute.
/// - `terminator` — mark as a block terminator.
#[derive(Debug, Clone, FromVariant)]
#[darling(attributes(kirin))]
pub struct StatementOptions {
    pub format: Option<String>,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub speculatable: bool,
    #[darling(default)]
    pub terminator: bool,
}

/// Parsed `#[kirin(...)]` attributes on a field within a statement struct or enum variant.
///
/// # Attributes
///
/// - `into` — call `.into()` on the builder argument before storing (allows ergonomic
///   type conversions).
/// - `default` / `default = <expr>` — provide a default value for the field. The bare
///   `#[kirin(default)]` form uses `Default::default()`; the expression form uses the
///   given expression.
/// - `type = <expr>` — the SSA type expression for this field's IR value
///   (e.g. `ArithType::Float`, `SimpleIRType::default()`).
#[derive(Debug, Clone, FromField)]
#[darling(attributes(kirin))]
pub struct KirinFieldOptions {
    #[darling(default)]
    pub into: bool,
    pub default: Option<DefaultValue>,
    #[darling(rename = "type")]
    pub ssa_ty: Option<syn::Expr>,
}

/// Default value specification for a field.
///
/// Supports two forms:
/// - `#[kirin(default)]` - Uses `Default::default()` as the default value
/// - `#[kirin(default = expr)]` - Uses the provided expression as the default value
#[derive(Debug, Clone)]
pub enum DefaultValue {
    /// Use `Default::default()` as the default value
    Default,
    /// Use a custom expression as the default value
    Expr(syn::Expr),
}

impl DefaultValue {
    /// Returns the expression to use as the default value.
    pub fn to_expr(&self) -> syn::Expr {
        match self {
            DefaultValue::Default => syn::parse_quote!(::core::default::Default::default()),
            DefaultValue::Expr(expr) => expr.clone(),
        }
    }
}

impl FromMeta for DefaultValue {
    fn from_word() -> darling::Result<Self> {
        Ok(DefaultValue::Default)
    }

    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        Ok(DefaultValue::Expr(expr.clone()))
    }
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

#[derive(Debug, Clone)]
pub struct GlobalOptions {
    pub crate_path: Option<syn::Path>,
    pub ir_type: syn::Path,
    pub builder: Option<BuilderOptions>,
    pub constant: bool,
    pub pure: bool,
    pub speculatable: bool,
    pub terminator: bool,
}

impl FromDeriveInput for GlobalOptions {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        match input.data {
            syn::Data::Struct(_) => {
                let opts = KirinStructOptions::from_derive_input(input)?;
                Ok(opts.into())
            }
            syn::Data::Enum(_) => {
                let opts = KirinEnumOptions::from_derive_input(input)?;
                Ok(opts.into())
            }
            _ => Err(Error::custom(
                "Kirin can only be derived for structs and enums",
            )),
        }
    }
}

impl FromDeriveInput for StatementOptions {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        let opts = KirinStructOptions::from_derive_input(input)?;
        Ok(opts.into())
    }
}

impl From<KirinStructOptions> for GlobalOptions {
    fn from(opts: KirinStructOptions) -> Self {
        GlobalOptions {
            crate_path: opts.crate_path,
            ir_type: opts.ir_type,
            builder: opts.builder,
            constant: opts.constant,
            pure: opts.pure,
            speculatable: opts.speculatable,
            terminator: opts.terminator,
        }
    }
}

impl From<KirinStructOptions> for StatementOptions {
    fn from(opts: KirinStructOptions) -> Self {
        StatementOptions {
            format: opts.format,
            builder: opts.builder,
            constant: opts.constant,
            pure: opts.pure,
            speculatable: opts.speculatable,
            terminator: opts.terminator,
        }
    }
}

impl From<KirinEnumOptions> for GlobalOptions {
    fn from(value: KirinEnumOptions) -> Self {
        GlobalOptions {
            crate_path: value.crate_path,
            ir_type: value.ir_type,
            builder: value.builder,
            constant: value.constant,
            pure: value.pure,
            speculatable: value.speculatable,
            terminator: value.terminator,
        }
    }
}
