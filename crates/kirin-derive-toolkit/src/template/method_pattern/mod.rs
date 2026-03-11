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
/// Each `MethodPattern` produces a `TokenStream` body for one method. The template
/// system calls `for_struct` when the derive input is a struct and `for_variant`
/// for each enum variant (the result becomes a match arm body).
///
/// Implementors include [`BoolProperty`], [`DelegateToWrapper`], [`FieldCollection`],
/// and [`Custom`] for ad-hoc logic.
pub trait MethodPattern<L: Layout> {
    /// Generate the method body for a struct derive input.
    fn for_struct(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream>;

    /// Generate the method body for a single enum variant (used as a match arm body).
    fn for_variant(
        &self,
        ctx: &DeriveContext<'_, L>,
        stmt: &StatementContext<'_, L>,
    ) -> darling::Result<TokenStream>;

    /// Return additional where-clause predicates required by this pattern.
    ///
    /// Defaults to an empty list. Override when the generated code needs trait
    /// bounds that are not already present on the impl.
    fn extra_bounds(
        &self,
        _ctx: &DeriveContext<'_, L>,
        _stmt: &StatementContext<'_, L>,
    ) -> Vec<syn::WherePredicate> {
        vec![]
    }
}

/// Specification for a single method inside a template-generated trait impl.
///
/// Groups the method signature (name, receiver, parameters, return type) together
/// with a [`MethodPattern`] that produces the body.
pub struct MethodSpec<L: Layout> {
    /// Method name (e.g., `is_pure`).
    pub name: syn::Ident,
    /// Receiver token (e.g., `&self`, `&mut self`).
    pub self_arg: TokenStream,
    /// Additional parameter declarations after the receiver.
    pub params: Vec<TokenStream>,
    /// Return type. `None` means the method returns `()`.
    pub return_type: Option<TokenStream>,
    /// Code generation strategy for the method body.
    pub pattern: Box<dyn MethodPattern<L>>,
    /// Optional method-level generic parameters (e.g., `<L: Dialect>`).
    pub generics: Option<TokenStream>,
    /// Optional method-level where clause.
    pub method_where_clause: Option<TokenStream>,
}

/// Specification for an associated type inside a template-generated trait impl.
///
/// Used by [`TraitImplTemplate`](super::TraitImplTemplate) to emit `type Foo = ...;`
/// items in the generated impl block.
pub enum AssocTypeSpec<L: Layout> {
    /// Fixed type shared across all variants (e.g., `type Output = bool;`).
    Fixed(syn::Ident, TokenStream),
    /// Type computed per-statement, allowing wrapper variants to delegate to the
    /// wrapped type's associated type via type inference.
    PerStatement {
        /// Associated type name.
        name: syn::Ident,
        /// Closure that produces the type tokens for a given statement.
        compute: Box<dyn Fn(&DeriveContext<'_, L>, &StatementContext<'_, L>) -> TokenStream>,
    },
}
