use darling::{FromDeriveInput, FromVariant};
use quote::ToTokens;

use super::field::Fields;
use super::{FieldMember, traits::*};

/// a dialect statement with its kirin attributes and fields
#[derive(Clone)]
pub struct Statement<'src, S, Ctx: Context<'src>> {
    pub src: &'src S,
    pub attrs: Ctx::AttrStatement,
    pub wraps: bool,
    pub fields: Fields<'src, Ctx::AttrField, Ctx::FieldExtra>,
    pub extra: Ctx::StatementExtra,
}

impl<'src, S, Ctx: Context<'src>> Statement<'src, S, Ctx> {
    /// Check if the statement is a wrapper
    pub fn is_wrapper(&self) -> bool {
        self.wraps
    }
}

impl<'src, S: ToTokens, Ctx: Context<'src>> Source for Statement<'src, S, Ctx> {
    type Output = S;
    fn source(&self) -> &Self::Output {
        self.src
    }
}

impl<'src, Ctx: Context<'src>> SourceIdent for Statement<'src, syn::Variant, Ctx> {
    fn source_ident(&self) -> &syn::Ident {
        &self.src.ident
    }
}

impl<'src, Ctx: Context<'src>> HasGenerics for Statement<'src, syn::DeriveInput, Ctx> {
    fn generics(&self) -> &syn::Generics {
        &self.src.generics
    }
}

impl<'src, S, Ctx: Context<'src>> ContainsWrapper for Statement<'src, S, Ctx> {
    fn contains_wrapper(&self) -> bool {
        self.wraps
    }
}

impl<'src, S, Ctx> Wrapper<'src, Ctx::AttrField, Ctx::FieldExtra> for Statement<'src, S, Ctx>
where
    S: ToTokens,
    Ctx: Context<'src>,
{
    fn wrapper(&self) -> Option<FieldMember<'_, 'src, Ctx::AttrField, Ctx::FieldExtra>> {
        self.fields.wrapper()
    }
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::DeriveInput>
    for Statement<'src, syn::DeriveInput, Ctx>
{
    fn from_context(ctx: &Ctx, node: &'src syn::DeriveInput) -> syn::Result<Self> {
        let syn::Data::Struct(data) = &node.data else {
            return Err(syn::Error::new_spanned(
                node,
                "expect struct input, got enum or union",
            ));
        };

        let mut wraps = node.attrs.iter().any(|attr| attr.path().is_ident("wraps"));
        let mut fields = Fields::from_context(ctx, &data.fields)?;
        let field_wraps = fields.wrapper();
        if wraps {
            fields.set_wrapper()?;
        } else if let Some(_) = field_wraps {
            wraps = true;
        }

        Ok(Statement {
            src: node,
            attrs: Ctx::AttrStatement::from_derive_input(node)?,
            wraps,
            fields,
            extra: Ctx::StatementExtra::from_context(ctx, node)?,
        })
    }
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::Variant>
    for Statement<'src, syn::Variant, Ctx>
{
    fn from_context(ctx: &Ctx, node: &'src syn::Variant) -> syn::Result<Self> {
        let mut wraps = node.attrs.iter().any(|attr| attr.path().is_ident("wraps"));
        let mut fields = Fields::from_context(ctx, &node.fields)?;
        let field_wraps = fields.wrapper();
        if wraps {
            fields.set_wrapper()?;
        } else if let Some(_) = field_wraps {
            wraps = true;
        }

        Ok(Statement {
            src: node,
            attrs: Ctx::AttrStatement::from_variant(node)?,
            wraps,
            fields,
            extra: Ctx::StatementExtra::from_context(ctx, node)?,
        })
    }
}

impl<'src, S, Ctx: Context<'src>> std::fmt::Debug for Statement<'src, S, Ctx>
where
    Ctx::AttrStatement: std::fmt::Debug,
    Ctx::AttrField: std::fmt::Debug,
    Ctx::FieldExtra: std::fmt::Debug,
    Ctx::StatementExtra: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Statement")
            .field("attrs", &self.attrs)
            .field("wraps", &self.wraps)
            .field("fields", &self.fields)
            .field("extra", &self.extra)
            .finish()
    }
}
