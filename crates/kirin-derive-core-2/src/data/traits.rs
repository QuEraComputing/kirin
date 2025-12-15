use crate::data::Dialect;

pub trait FromContext<'src, Ctx: Context<'src>, Node>: Sized {
    fn from_context(ctx: &Ctx, node: &'src Node) -> syn::Result<Self>;
}

impl<'src, Ctx: Context<'src>, Node> FromContext<'src, Ctx, Node> for () {
    fn from_context(_ctx: &Ctx, _node: &Node) -> syn::Result<Self> {
        Ok(())
    }
}

pub trait Context<'src>: Sized {
    /// Extra data for the helper attribute per statement or global
    type AttrGlobal: darling::FromDeriveInput;
    type AttrStatement: darling::FromDeriveInput + darling::FromVariant;
    type AttrField: darling::FromField;
    /// Extra data for each field of a statement
    type FieldExtra: FromContext<'src, Self, syn::Field>;
    /// Extra data for each statement in the dialect
    type StatementExtra: FromContext<'src, Self, syn::DeriveInput>
        + FromContext<'src, Self, syn::Variant>;

    fn helper_attribute() -> &'static str {
        "kirin"
    }
    fn crate_path(&self) -> &syn::Path;
}

impl Context<'_> for () {
    type AttrGlobal = ();
    type AttrStatement = ();
    type AttrField = ();
    type FieldExtra = ();
    type StatementExtra = ();

    fn helper_attribute() -> &'static str {
        "kirin"
    }

    fn crate_path(&self) -> &syn::Path {
        panic!("crate_path called on unit context")
    }
}

pub trait SimpleTraitDerive<'src>: Context<'src> {
    fn trait_name(&self) -> &syn::Ident;
    fn trait_method(&self) -> &syn::Ident;
    fn trait_generics(&self) -> &syn::Generics;
    fn trait_impl(&'src self, data: &Dialect<'src, Self>) -> super::gadgets::TraitImpl<'src> {
        let input = data.input();
        super::gadgets::TraitImpl::new(input, self.trait_name(), self.trait_generics())
    }
}

/// Compile a data node into an intermediate representation
/// that implements ToTokens
pub trait Compile<'src, Ctx: Context<'src>, Node>: Sized {
    fn compile(ctx: &'src Ctx, node: &'src Node) -> syn::Result<Self>;
}
