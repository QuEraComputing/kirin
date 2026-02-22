extern crate proc_macro;

mod call_semantics;
mod interpretable;

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

#[proc_macro_derive(Interpretable, attributes(wraps, kirin))]
pub fn derive_interpretable(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match interpretable::DeriveInterpretable::default().emit(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}
