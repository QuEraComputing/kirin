extern crate proc_macro;

mod eval_call;
mod interpretable;

use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(EvalCall, attributes(wraps, callable, kirin))]
pub fn derive_eval_call(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match eval_call::DeriveEvalCall::default().emit(&ast) {
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
