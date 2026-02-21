extern crate proc_macro;

mod call_semantics;

use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(CallSemantics, attributes(wraps, kirin))]
pub fn derive_call_semantics(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match call_semantics::DeriveCallSemantics::default().emit(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}
