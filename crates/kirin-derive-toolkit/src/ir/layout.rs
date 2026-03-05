use darling::{FromDeriveInput, FromField, FromVariant};

pub trait Layout {
    type StatementExtra: FromDeriveInput + FromVariant + Clone + std::fmt::Debug;
    type ExtraGlobalAttrs: FromDeriveInput + Clone + std::fmt::Debug;
    type ExtraStatementAttrs: FromDeriveInput + FromVariant + Clone + std::fmt::Debug;
    type ExtraFieldAttrs: FromField + Clone + std::fmt::Debug;
}

#[derive(Debug, Clone)]
pub struct StandardLayout;

impl Layout for StandardLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = ();
    type ExtraStatementAttrs = ();
    type ExtraFieldAttrs = ();
}
