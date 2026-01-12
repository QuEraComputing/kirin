mod iter_def;
mod iterator_impl;
mod pattern;
mod trait_impl;
mod wrapper;

pub use iter_def::{IterEnumDefTokens, IterStructDefTokens, VariantDefTokens};
pub use iterator_impl::IteratorImplTokens;
pub use pattern::FieldPatternTokens;
pub use trait_impl::{TraitAssocTypeImplTokens, TraitImplTokens, TraitMethodImplTokens};
pub use wrapper::{WrapperCallTokens, WrapperIterTypeTokens};

use proc_macro2::TokenStream;
use quote::ToTokens;

pub fn to_stream<T: ToTokens>(value: T) -> TokenStream {
    value.to_token_stream()
}
