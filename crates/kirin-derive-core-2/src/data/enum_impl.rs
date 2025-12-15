use darling::FromDeriveInput;

use super::core::Statement;
use super::traits::{Context, FromContext};

pub struct DialectEnum<'src, Ctx: Context<'src>> {
    pub attrs: Ctx::AttrGlobal,
    pub src: &'src syn::DeriveInput,
    pub variants: Vec<Statement<'src, syn::Variant, Ctx>>,
}

impl<'src, Ctx: Context<'src>> DialectEnum<'src, Ctx> {
    pub fn input(&self) -> &'src syn::DeriveInput {
        self.src
    }
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::DeriveInput> for DialectEnum<'src, Ctx> {
    fn from_context(ctx: &Ctx, node: &'src syn::DeriveInput) -> syn::Result<Self> {
        let syn::Data::Enum(data) = &node.data else {
            return Err(syn::Error::new_spanned(
                node,
                "DialectEnum can only be created from enum data",
            ));
        };

        Ok(DialectEnum {
            attrs: Ctx::AttrGlobal::from_derive_input(node)?,
            src: node,
            variants: data
                .variants
                .iter()
                .map(|variant| Statement::from_context(ctx, variant))
                .collect::<syn::Result<Vec<_>>>()?,
        })
    }
}

impl<'src, Ctx: Context<'src>> std::fmt::Debug for DialectEnum<'src, Ctx>
where
    Ctx::AttrGlobal: std::fmt::Debug,
    Statement<'src, syn::Variant, Ctx>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialectEnum")
            .field("attrs", &self.attrs)
            .field("variants", &self.variants)
            .finish()
    }
}
