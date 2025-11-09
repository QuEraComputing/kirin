extern crate proc_macro;

use kirin_derive_core::{
    DeriveHasArguments, DeriveHasRegions, DeriveHasResults, DeriveHasSuccessors, DeriveInstruction,
    DeriveIsConstant, DeriveIsPure, DeriveIsTerminator, DeriveTrait,
};
use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(Instruction, attributes(kirin))]
pub fn derive_instruction(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveInstruction::generate(ast).into()
}

#[proc_macro_derive(HasArguments, attributes(kirin))]
pub fn derive_has_arguments(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveHasArguments::generate(ast).into()
}

#[proc_macro_derive(HasResults, attributes(kirin))]
pub fn derive_has_results(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveHasResults::generate(ast).into()
}

#[proc_macro_derive(HasSuccessors, attributes(kirin))]
pub fn derive_has_successors(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveHasSuccessors::generate(ast).into()
}

#[proc_macro_derive(HasRegions, attributes(kirin))]
pub fn derive_has_regions(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveHasRegions::generate(ast).into()
}

#[proc_macro_derive(IsTerminator, attributes(kirin))]
pub fn derive_is_terminator(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveIsTerminator::generate(ast).into()
}

#[proc_macro_derive(IsConstant, attributes(kirin))]
pub fn derive_is_constant(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveIsConstant::generate(ast).into()
}

#[proc_macro_derive(IsPure, attributes(kirin))]
pub fn derive_is_pure(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveIsPure::generate(ast).into()
}
