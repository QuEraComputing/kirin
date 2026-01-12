use proc_macro2::TokenStream;
use quote::ToTokens;

use super::{definition::*, source::Source};

pub trait AnyWrapper {
    /// returns true if the structure is a wrapper over another dialect/statement
    #[must_use]
    fn any_wrapper(&self) -> bool;
}

pub trait Wrapper<'src, L: Layout>: Source {
    /// get the wrapper field if any
    #[must_use]
    fn wrapper(&self) -> Option<Field<'_, 'src, L>>;

    #[must_use]
    fn wrapper_type(&self) -> Option<&'src syn::Type> {
        self.wrapper().map(|f| &f.src.ty)
    }

    #[must_use]
    fn wrapper_or_error(&self) -> syn::Result<Field<'_, 'src, L>> {
        self.wrapper().ok_or_else(|| {
            syn::Error::new_spanned(
                self.source(), // assuming Source trait is in scope
                "Expected a wrapper field but none was found",
            )
        })
    }

    #[must_use]
    fn wrapper_type_or_error(&self) -> syn::Result<&'src syn::Type> {
        self.wrapper_or_error().map(|f| &f.src.ty)
    }

    #[must_use]
    fn wrapper_tokens(&self) -> TokenStream {
        self.wrapper_or_error()
            .map(|f| f.to_token_stream())
            .unwrap_or_else(|e| e.to_compile_error())
    }

    #[must_use]
    fn wrapper_type_tokens(&self) -> TokenStream {
        self.wrapper_type_or_error()
            .map(|ty| ty.to_token_stream())
            .unwrap_or_else(|e| e.to_compile_error())
    }
}

impl<'src, L> Wrapper<'src, L> for Fields<'_, 'src, L>
where
    L: Layout,
{
    fn wrapper(&self) -> Option<Field<'_, 'src, L>> {
        let definition = self.definition();
        definition
            .fields()
            .iter()
            .zip(self.source().iter())
            .enumerate()
            .find(|(_, (f, _))| f.wraps)
            .map(|(i, (_, src))| Field {
                input: self.input,
                src,
                parent: definition.clone(),
                index: i,
            })
    }
}

impl<'src, L> Wrapper<'src, L> for Variant<'_, 'src, L>
where
    L: Layout,
{
    fn wrapper<'a>(&'a self) -> Option<Field<'a, 'src, L>> {
        let definition = self.definition();
        definition
            .fields
            .iter()
            .zip(&self.src.fields)
            .enumerate()
            .find(|(_, (f, _))| f.wraps)
            .map(|(i, (_, src))| Field {
                input: self.input,
                src,
                parent: self.into(),
                index: i,
            })
    }
}

impl<'src, L> Wrapper<'src, L> for Struct<'src, L>
where
    L: Layout,
{
    fn wrapper(&self) -> Option<Field<'_, 'src, L>> {
        self.definition()
            .fields
            .iter()
            .zip(&self.src.fields)
            .enumerate()
            .find(|(_, (f, _))| f.wraps)
            .map(|(i, (_, src))| Field {
                input: self.input,
                src,
                parent: self.into(),
                index: i,
            })
    }
}

impl<L: Layout> AnyWrapper for Input<'_, L> {
    fn any_wrapper(&self) -> bool {
        match self {
            Input::Struct(s) => s.any_wrapper(),
            Input::Enum(e) => e.any_wrapper(),
        }
    }
}

impl<L: Layout> AnyWrapper for Struct<'_, L> {
    // struct is a single statement, so just check if it is a wrapper
    fn any_wrapper(&self) -> bool {
        self.is_wrapper()
    }
}

impl<L: Layout> AnyWrapper for Enum<'_, L> {
    fn any_wrapper(&self) -> bool {
        // happy path: the enum is a wrapper - all its variants are wrappers
        // otherwise: enum may have multiple variants, if one of them is a wrapper, return true
        self.definition().marked_wraps || self.variants().any(|v| v.is_wrapper())
    }
}
