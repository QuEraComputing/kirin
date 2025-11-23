use proc_macro2::TokenStream;

use super::super::info::FieldIterInfo;
use crate::data::*;

pub trait IteratorNextArm {
    fn generate_iterator_next_arm(&self, iter_name: &syn::Ident) -> TokenStream;
}

macro_rules! impl_iter_next_arm {
    ($variant:ident) => {
        impl IteratorNextArm for $variant<'_, FieldIterInfo> {
            fn generate_iterator_next_arm(&self, iter_name: &syn::Ident) -> TokenStream {
                let variant_name = self.variant_name;
                quote::quote! {
                    #iter_name::#variant_name ( iter ) => {
                        iter.next()
                    }
                }
            }
        }
    };
}

impl_iter_next_arm!(RegularVariant);
impl_iter_next_arm!(NamedWrapperVariant);
impl_iter_next_arm!(UnnamedWrapperVariant);

impl IteratorNextArm for EitherVariant<'_, FieldIterInfo> {
    fn generate_iterator_next_arm(&self, iter_name: &syn::Ident) -> TokenStream {
        match &self {
            EitherVariant::Regular(data) => data.generate_iterator_next_arm(iter_name),
            EitherVariant::Wrapper(data) => data.generate_iterator_next_arm(iter_name),
        }
    }
}

impl IteratorNextArm for WrapperVariant<'_, FieldIterInfo> {
    fn generate_iterator_next_arm(&self, iter_name: &syn::Ident) -> TokenStream {
        match &self {
            WrapperVariant::Named(data) => data.generate_iterator_next_arm(iter_name),
            WrapperVariant::Unnamed(data) => data.generate_iterator_next_arm(iter_name),
        }
    }
}
