use darling::FromDeriveInput;

use super::core::Statement;
use super::{FieldMember, traits::*};

pub struct DialectStruct<'src, Ctx: Context<'src>> {
    pub attrs: Ctx::AttrGlobal,
    pub wraps: bool,
    pub statement: Statement<'src, syn::DeriveInput, Ctx>,
}

impl<'src, Ctx: Context<'src>> Wrapper<'src, Ctx::AttrField, Ctx::FieldExtra>
    for DialectStruct<'src, Ctx>
{
    fn wrapper(&self) -> Option<FieldMember<'_, 'src, Ctx::AttrField, Ctx::FieldExtra>> {
        self.statement.fields.wrapper()
    }
}

impl<'src, Ctx: Context<'src>> Source for DialectStruct<'src, Ctx> {
    type Output = syn::DeriveInput;
    fn source(&self) -> &Self::Output {
        self.statement.source()
    }
}

impl<'src, Ctx: Context<'src>> HasGenerics for DialectStruct<'src, Ctx> {
    fn generics(&self) -> &syn::Generics {
        &self.statement.src.generics
    }
}

impl<'src, Ctx: Context<'src>> ContainsWrapper for DialectStruct<'src, Ctx> {
    fn contains_wrapper(&self) -> bool {
        self.wraps
    }
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::DeriveInput>
    for DialectStruct<'src, Ctx>
{
    fn from_context(ctx: &Ctx, node: &'src syn::DeriveInput) -> syn::Result<Self> {
        let mut wraps = node.attrs.iter().any(|attr| attr.path().is_ident("wraps"));
        let mut statement = Statement::from_context(ctx, node)?;
        if wraps {
            statement.wraps = true;
            statement.fields.set_wrapper()?;
        }

        if statement.wraps {
            wraps = true;
        }

        Ok(DialectStruct {
            attrs: Ctx::AttrGlobal::from_derive_input(node)?,
            wraps,
            statement,
        })
    }
}

impl<'src, Ctx: Context<'src>> std::fmt::Debug for DialectStruct<'src, Ctx>
where
    Ctx::AttrGlobal: std::fmt::Debug,
    Statement<'src, syn::DeriveInput, Ctx>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialectStruct")
            .field("wraps", &self.wraps)
            .field("attrs", &self.attrs)
            .field("statement", &self.statement)
            .finish()
    }
}
