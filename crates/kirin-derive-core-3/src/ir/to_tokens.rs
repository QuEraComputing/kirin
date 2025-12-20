use quote::{ToTokens, format_ident};

use super::definition::*;

impl<'a, 'src, L: Layout> ToTokens for Field<'a, 'src, L> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.src
            .ident
            .as_ref()
            .unwrap_or(&format_ident!("field_{}", self.index))
            .to_tokens(tokens);
    }
}
