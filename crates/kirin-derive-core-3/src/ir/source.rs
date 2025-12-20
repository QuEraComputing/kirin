use quote::{ToTokens, format_ident};

use super::definition::*;

pub trait Source {
    type Output: ToTokens;
    #[must_use]
    fn source(&self) -> Self::Output;
}

impl<'src, L: Layout> Source for Input<'src, L> {
    type Output = &'src syn::DeriveInput;
    fn source(&self) -> Self::Output {
        match self {
            Input::Struct(s) => s.source(),
            Input::Enum(e) => e.source(),
        }
    }
}

impl<'src, L: Layout> Source for Struct<'src, L> {
    type Output = &'src syn::DeriveInput;
    fn source(&self) -> Self::Output {
        self.input
    }
}

impl<'src, L: Layout> Source for Enum<'src, L> {
    type Output = &'src syn::DeriveInput;
    fn source(&self) -> Self::Output {
        self.input
    }
}

impl<'src, L: Layout> Source for Variant<'_, 'src, L> {
    type Output = &'src syn::Variant;
    fn source(&self) -> Self::Output {
        self.src
    }
}

impl<'a, 'src, L: Layout> Source for Field<'a, 'src, L> {
    type Output = &'src syn::Field;
    fn source(&self) -> Self::Output {
        self.src
    }
}

impl<'a, 'src, L: Layout> Source for Fields<'a, 'src, L> {
    type Output = &'src syn::Fields;
    fn source(&self) -> Self::Output {
        &self.src
    }
}

pub trait SourceIdent {
    fn source_ident(&self) -> syn::Ident;
}

impl<'src, L: Layout> SourceIdent for Input<'src, L> {
    fn source_ident(&self) -> syn::Ident {
        match self {
            Input::Struct(s) => s.input.ident.clone(),
            Input::Enum(e) => e.input.ident.clone(),
        }
    }
}

impl<'src, L: Layout> SourceIdent for Struct<'src, L> {
    fn source_ident(&self) -> syn::Ident {
        self.input.ident.clone()
    }
}

impl<'src, L: Layout> SourceIdent for Enum<'src, L> {
    fn source_ident(&self) -> syn::Ident {
        self.input.ident.clone()
    }
}

impl<'src, L: Layout> SourceIdent for Variant<'_, 'src, L> {
    fn source_ident(&self) -> syn::Ident {
        self.src.ident.clone()
    }
}

impl<'a, 'src, L: Layout> SourceIdent for Fields<'a, 'src, L> {
    fn source_ident(&self) -> syn::Ident {
        self.ident.clone()
    }
}

impl<'a, 'src, L: Layout> SourceIdent for Field<'a, 'src, L> {
    fn source_ident(&self) -> syn::Ident {
        self.src
            .ident
            .clone()
            .unwrap_or_else(|| format_ident!("field_{}", self.index))
    }
}

pub trait WithInput<'src> {
    fn input(&self) -> &'src syn::DeriveInput;
}

impl<'src, L: Layout> WithInput<'src> for Struct<'src, L> {
    fn input(&self) -> &'src syn::DeriveInput {
        self.input
    }
}

impl<'src, L: Layout> WithInput<'src> for Enum<'src, L> {
    fn input(&self) -> &'src syn::DeriveInput {
        self.input
    }
}

impl<'a, 'src, L: Layout> WithInput<'src> for Field<'a, 'src, L> {
    fn input(&self) -> &'src syn::DeriveInput {
        self.input
    }
}

impl<'a, 'src, L: Layout> WithInput<'src> for Fields<'a, 'src, L> {
    fn input(&self) -> &'src syn::DeriveInput {
        self.input
    }
}

impl<'a, 'src, L: Layout> WithInput<'src> for Variant<'a, 'src, L> {
    fn input(&self) -> &'src syn::DeriveInput {
        self.input
    }
}

impl<'src, L: Layout> WithInput<'src> for Input<'src, L> {
    fn input(&self) -> &'src syn::DeriveInput {
        match self {
            Input::Struct(s) => s.input(),
            Input::Enum(e) => e.input(),
        }
    }
}
