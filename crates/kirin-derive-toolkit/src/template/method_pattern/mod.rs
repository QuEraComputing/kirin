pub mod bool_property;
pub mod builder_pattern;
pub mod custom;
pub mod delegate;
pub mod field_collection;

pub use bool_property::BoolProperty;
pub use builder_pattern::BuilderPattern;
pub use custom::Custom;
pub use delegate::{DelegateToWrapper, SelectiveDelegation};
pub use field_collection::FieldCollection;

use crate::context::{DeriveContext, StatementContext};
use crate::ir::Layout;
use proc_macro2::TokenStream;

/// Per-variant code generation logic for a single method in a trait impl.
///
/// `for_struct` handles the struct case (single body).
/// `for_variant` handles each enum variant (becomes a match arm body).
pub trait MethodPattern<L: Layout> {
    fn for_struct(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream>;

    fn for_variant(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream>;

    fn extra_bounds(
        &self,
        _ctx: &DeriveContext<'_, L>,
        _stmt: &StatementContext<'_, L>,
    ) -> Vec<syn::WherePredicate> {
        vec![]
    }
}

/// Specification for a single method inside a template-generated trait impl.
pub struct MethodSpec<L: Layout> {
    pub name: syn::Ident,
    pub self_arg: TokenStream,
    pub params: Vec<TokenStream>,
    pub return_type: Option<TokenStream>,
    pub pattern: Box<dyn MethodPattern<L>>,
}

/// Specification for an associated type inside a template-generated trait impl.
pub enum AssocTypeSpec<L: Layout> {
    /// Fixed type for all variants.
    Fixed(syn::Ident, TokenStream),
    /// Dynamic type computed per-statement (uses first wrapper for type inference).
    PerStatement {
        name: syn::Ident,
        compute: Box<dyn Fn(&DeriveContext<'_, L>, &StatementContext<'_, L>) -> TokenStream>,
    },
}
