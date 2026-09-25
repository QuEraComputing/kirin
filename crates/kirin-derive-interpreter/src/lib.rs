extern crate proc_macro;

mod frame;
mod interp_dispatch;
mod interpretable;

use proc_macro::TokenStream;
use syn::parse_macro_input;

/// Derive `Interpretable<I>` for a `#[wraps]` wrapper enum by delegating to
/// each wrapped statement's `Interpretable` impl.
#[proc_macro_derive(Interpretable, attributes(wraps, kirin, interpret))]
pub fn derive_interpretable(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match interpretable::do_derive_interpretable(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}

/// Derive `InterpDispatch<I>` for a stage enum, dispatching statement
/// interpretation to each stage's language. Uses the same
/// `#[stage(...)]` attributes as `StageMeta` / `ParseDispatch`.
#[proc_macro_derive(InterpDispatch, attributes(stage))]
pub fn derive_interp_dispatch(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match interp_dispatch::generate(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

/// Derive stack-item `Frame` dispatch and `From<Member>` for a nonempty enum
/// of single-field tuple variants. All members must share a completion type.
#[proc_macro_derive(Frame)]
pub fn derive_frame(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match frame::generate(&ast) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
