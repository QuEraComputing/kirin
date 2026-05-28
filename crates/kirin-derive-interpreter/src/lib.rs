extern crate proc_macro;

mod frame;
mod function_entry;
mod interpretable;
mod layout;
mod stage_frame;

use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(Interpretable, attributes(wraps, kirin, interpret))]
pub fn derive_interpretable(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match interpretable::do_derive_interpretable(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}

#[proc_macro_derive(FunctionEntry, attributes(wraps, callable, kirin, interpret))]
pub fn derive_function_entry(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match function_entry::do_derive_function_entry(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}

#[proc_macro_derive(HasLocation, attributes(interpret))]
pub fn derive_has_location(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match frame::do_derive_has_location(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

#[proc_macro_derive(Frame, attributes(kirin, interpret))]
pub fn derive_frame(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match frame::do_derive_frame(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

#[proc_macro_derive(Completion, attributes(kirin, interpret))]
pub fn derive_completion(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match frame::do_derive_completion(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

#[proc_macro_derive(LiftError, attributes(kirin))]
pub fn derive_lift_error(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match frame::do_derive_lift_error(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

#[proc_macro_derive(StageFrame, attributes(stage_frame, interpret))]
pub fn derive_stage_frame(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match stage_frame::do_derive_stage_frame(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}
