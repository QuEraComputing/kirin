use std::ops::Deref;

use super::traits::{Context, FromContext};
use bon::Builder;
use darling::FromField;
use quote::{ToTokens, format_ident, quote};

#[derive(Clone)]
pub struct Fields<'src, Attr, E = ()> {
    src: &'src syn::Fields,
    inner: Vec<Field<'src, Attr, E>>,
}

impl<'src, Attr, E> Fields<'src, Attr, E> {
    /// number of fields
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// check if there is no field
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// iterator over the fields, yielding `FieldMember`s
    /// allowing to be used in `ToTokens` implementations
    pub fn iter(&self) -> FieldsIter<'_, 'src, Attr, E> {
        FieldsIter {
            fields: self,
            current: 0,
        }
    }

    /// unpacking pattern for the fields
    /// e.g. `{ field1, field2 }` or `(field0, field1)` or just `` for unit fields
    pub fn unpacking(&self) -> Unpacking<'_, 'src, Attr, E> {
        Unpacking { inner: self }
    }

    /// returns the field marked as wraps, if any
    pub fn wrapper(&self) -> Option<FieldMember<'_, 'src, Attr, E>> {
        let (idx, f) = self.inner.iter().enumerate().find(|(_, f)| f.wraps)?;
        Some(FieldMember {
            data: f,
            index: idx,
        })
    }

    /// set the field marked as wraps, if only one field exists
    pub fn set_wrapper(&mut self) -> Result<(), syn::Error> {
        if self.len() == 1 {
            self.inner[0].wraps = true;
            Ok(())
        } else {
            Err(syn::Error::new_spanned(
                self.src,
                "Cannot infer wrapper field: #[wraps] is set but no field is marked as wraps",
            ))
        }
    }
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::Fields>
    for Fields<'src, Ctx::AttrField, Ctx::FieldExtra>
{
    fn from_context(ctx: &Ctx, node: &'src syn::Fields) -> syn::Result<Self> {
        Ok(Fields {
            src: node,
            inner: node
                .iter()
                .map(|f| Field::from_context(ctx, f))
                .collect::<syn::Result<Vec<_>>>()?,
        })
    }
}

impl<'src, Attr, E> std::fmt::Debug for Fields<'src, Attr, E>
where
    Attr: std::fmt::Debug,
    E: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldMembers")
            .field("fields", &self.inner)
            .finish()
    }
}

pub struct FieldsIter<'a, 'src, Attr, E = ()> {
    fields: &'a Fields<'src, Attr, E>,
    current: usize,
}

impl<'a, 'src: 'a, Attr: 'src, E: 'src> Iterator for FieldsIter<'a, 'src, Attr, E> {
    type Item = FieldMember<'a, 'src, Attr, E>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.fields.len() {
            return None;
        }
        let index = self.current;
        let field = &self.fields.inner[self.current];
        self.current += 1;
        Some(FieldMember { data: field, index })
    }
}

pub struct FieldMember<'a, 'src, Attr, E = ()> {
    data: &'a Field<'src, Attr, E>,
    pub index: usize,
}

impl<'a, 'src, Attr, E> ToTokens for FieldMember<'a, 'src, Attr, E> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.data
            .src
            .ident
            .as_ref()
            .unwrap_or(&format_ident!("field_{}", self.index))
            .to_tokens(tokens);
    }
}

impl<'a, 'src, Attr, E> Deref for FieldMember<'a, 'src, Attr, E> {
    type Target = Field<'src, Attr, E>;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

pub struct Unpacking<'a, 'src, Attr, E = ()> {
    inner: &'a Fields<'src, Attr, E>,
}

impl<'a, 'src, Attr, E> ToTokens for Unpacking<'a, 'src, Attr, E> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let inner = self.inner.iter();
        match self.inner.src {
            syn::Fields::Named(_) => {
                quote! { { #(#inner),* }  }
            }
            syn::Fields::Unnamed(_) => {
                quote! { ( #(#inner),* ) }
            }
            syn::Fields::Unit => {
                quote! {}
            }
        }
        .to_tokens(tokens);
    }
}

/// a field of a statement with its kirin attributes
/// can be extended with extra data via the generic parameter `E`
#[derive(Clone, Builder)]
pub struct Field<'src, Attr, E = ()> {
    pub src: &'src syn::Field,
    pub wraps: bool,
    pub attrs: Attr,
    pub extra: E,
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::Field>
    for Field<'src, Ctx::AttrField, Ctx::FieldExtra>
{
    fn from_context(ctx: &Ctx, node: &'src syn::Field) -> syn::Result<Self> {
        Ok(Field {
            src: node,
            wraps: node.attrs.iter().any(|attr| attr.path().is_ident("wraps")),
            attrs: Ctx::AttrField::from_field(node)?,
            extra: Ctx::FieldExtra::from_context(ctx, node)?,
        })
    }
}

impl<'src, Attr, E> std::fmt::Debug for Field<'src, Attr, E>
where
    Attr: std::fmt::Debug,
    E: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("wraps", &self.wraps)
            .field("ident", &self.src.ident)
            .field("ty", &self.src.ty)
            .field("attrs", &self.attrs)
            .field("extra", &self.extra)
            .finish()
    }
}
