use quote::quote;

use crate::ir::{Enum, HasFields, Layout, Source, Struct, Variant};
use crate::{derive::Compile, target};

target! {
    /// Returns the unpacking patterns for all variants in the enum.
    ///
    /// !!! Note
    /// Usually used in conjunction with `variant_idents` to form match arms.
    pub struct Unpacking
}

impl<'src, Node, L> Compile<'src, Node, Unpacking> for L
where
    L: Layout,
    Node: HasFields<'src, L>,
{
    fn compile(&self, node: &Node) -> Unpacking {
        let fields = node.fields();
        let inner = fields.iter();
        match fields.source() {
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
        .into()
    }
}

impl<'src, L: Layout> Struct<'src, L> {
    pub fn unpacking(&self) -> Unpacking {
        unpacking_private(self)
    }
}

impl<'src, L: Layout> Enum<'src, L> {
    pub fn unpacking(&'src self) -> Vec<Unpacking> {
        self.variants()
            .map(|v| v.unpacking())
            .collect::<Vec<Unpacking>>()
    }
}

impl<'a, 'src, L: Layout> Variant<'a, 'src, L> {
    pub fn unpacking(&self) -> Unpacking {
        unpacking_private(self)
    }
}

fn unpacking_private<'src, L, Node>(node: &Node) -> Unpacking
where
    L: Layout,
    Node: HasFields<'src, L>,
{
    let fields = node.fields();
    let inner = fields.iter();
    match fields.source() {
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
    .into()
}
