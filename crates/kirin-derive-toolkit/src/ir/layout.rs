use darling::{FromDeriveInput, FromField, FromVariant};

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
    type ExtraStatementAttrs: FromDeriveInput + FromVariant + Clone + std::fmt::Debug;
    type ExtraFieldAttrs: FromField + Clone + std::fmt::Debug;
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
}
