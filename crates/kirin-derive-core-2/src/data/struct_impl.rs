use darling::FromDeriveInput;

use super::core::Statement;
use super::traits::{Context, FromContext};

pub struct DialectStruct<'src, Ctx: Context<'src>> {
    pub attrs: Ctx::AttrGlobal,
    pub statement: Statement<'src, syn::DeriveInput, Ctx>,
}

impl<'src, Ctx: Context<'src>> DialectStruct<'src, Ctx> {
    pub fn input(&self) -> &'src syn::DeriveInput {
        self.statement.src
    }
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::DeriveInput>
    for DialectStruct<'src, Ctx>
{
    fn from_context(ctx: &Ctx, node: &'src syn::DeriveInput) -> syn::Result<Self> {
        Ok(DialectStruct {
            attrs: Ctx::AttrGlobal::from_derive_input(node)?,
            statement: Statement::from_context(ctx, node)?,
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
            .field("attrs", &self.attrs)
            .field("statement", &self.statement)
            .finish()
    }
}
