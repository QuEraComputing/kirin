use darling::{FromDeriveInput, FromField, FromVariant};

/// Downstream derive macros implement this on their `ExtraGlobalAttrs`
/// to declare their crate path override (e.g. `#[chumsky(crate = ...)]`).
pub trait HasCratePath {
    fn crate_path(&self) -> Option<&syn::Path>;
}

impl HasCratePath for () {
    fn crate_path(&self) -> Option<&syn::Path> {
        None
    }
}

/// Extension point for per-derive custom attributes.
///
/// Each associated type corresponds to a level in the IR hierarchy where
/// a derive macro can inject extra parsed attributes. [`StandardLayout`]
/// sets all extras to `()` — use it unless your derive needs custom
/// attributes like `#[callable]` or `#[format(...)]`.
///
/// # Custom Layout Example
///
/// ```ignore
/// struct MyLayout;
///
/// impl Layout for MyLayout {
///     type StatementExtra = MyStatementAttrs;  // parsed from variant attrs
///     type ExtraGlobalAttrs = ();
///     type ExtraStatementAttrs = ();
///     type ExtraFieldAttrs = ();
/// }
/// ```
pub trait Layout {
    type StatementExtra: FromDeriveInput + FromVariant + Clone + std::fmt::Debug;
    type ExtraGlobalAttrs: FromDeriveInput + Clone + std::fmt::Debug;
    type ExtraStatementAttrs: FromVariant + Clone + std::fmt::Debug;
    type ExtraFieldAttrs: FromField + Clone + std::fmt::Debug;

    /// Parse statement-level extra attrs from a top-level derive input.
    ///
    /// Called for struct-shaped inputs where there are no enum variants.
    /// Implementations can filter out global-only fields (e.g. `crate`) that
    /// share the same attribute namespace but don't belong at the statement level.
    fn extra_statement_attrs_from_input(
        input: &syn::DeriveInput,
    ) -> darling::Result<Self::ExtraStatementAttrs>;
}

/// Default layout with no custom attributes at any level.
///
/// Use this for derives that only need the built-in `#[kirin(...)]` attributes.
#[derive(Debug, Clone)]
pub struct StandardLayout;

impl Layout for StandardLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = ();
    type ExtraStatementAttrs = ();
    type ExtraFieldAttrs = ();

    fn extra_statement_attrs_from_input(
        _input: &syn::DeriveInput,
    ) -> darling::Result<()> {
        Ok(())
    }
}
