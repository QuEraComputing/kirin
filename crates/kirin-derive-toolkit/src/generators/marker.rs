use crate::ir::{self, Layout};
use crate::tokens::TraitImpl;
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;

/// Generates a marker trait impl with a `Type` associated type alias.
///
/// Used to stamp the IR type identity onto dialect types.
pub fn derive_marker<L: Layout>(input: &ir::Input<L>, trait_path: &syn::Path) -> TokenStream {
    let ir_type = &input.attrs.ir_type;
    TraitImpl::new(input.generics.clone(), trait_path, &input.name)
        .assoc_type(syn::Ident::new("Type", Span::call_site()), ir_type)
        .to_token_stream()
}
