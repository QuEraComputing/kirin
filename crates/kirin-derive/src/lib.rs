extern crate proc_macro;

use kirin_derive_core::{derive_from, prelude::*};
use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(Statement, attributes(kirin))]
pub fn derive_statement(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let arguments = derive_field_iter!(&ast, "arguments", SSAValue, HasArguments);
    let arguments_mut = derive_field_iter_mut!(&ast, "arguments_mut", SSAValue, HasArgumentsMut);
    let results = derive_field_iter!(&ast, "results", ResultValue, HasResults);
    let results_mut = derive_field_iter_mut!(&ast, "results_mut", ResultValue, HasResultsMut);
    let successors = derive_field_iter!(&ast, "successors", Block, HasSuccessors);
    let successors_mut = derive_field_iter_mut!(&ast, "successors_mut", Block, HasSuccessorsMut);
    let regions = derive_field_iter!(&ast, "regions", Region, HasRegions);
    let regions_mut = derive_field_iter_mut!(&ast, "regions_mut", Region, HasRegionsMut);
    let is_terminator = derive_check!(&ast, is_terminator, IsTerminator);
    let is_constant = derive_check!(&ast, is_constant, IsConstant);
    let is_pure = derive_check!(&ast, is_pure, IsPure);
    let from = derive_from!(&ast);
    let statement = derive_empty!(&ast, Statement, ::kirin::ir);

    let generated = quote::quote! {
        #arguments
        #arguments_mut
        #results
        #results_mut
        #successors
        #successors_mut
        #regions
        #regions_mut
        #is_terminator
        #is_constant
        #is_pure
        #from

        #statement
    };
    generated.into()
}

#[proc_macro_derive(HasArguments, attributes(kirin))]
pub fn derive_has_arguments(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_field_iter!(&ast, "arguments", SSAValue, HasArguments).into()
}

#[proc_macro_derive(HasArgumentsMut, attributes(kirin))]
pub fn derive_has_arguments_mut(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_field_iter_mut!(&ast, "arguments_mut", SSAValue, HasArgumentsMut).into()
}

#[proc_macro_derive(HasResults, attributes(kirin))]
pub fn derive_has_results(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_field_iter!(&ast, "results", ResultValue, HasResults).into()
}

#[proc_macro_derive(HasResultsMut, attributes(kirin))]
pub fn derive_has_results_mut(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_field_iter_mut!(&ast, "results_mut", ResultValue, HasResultsMut).into()
}

#[proc_macro_derive(HasSuccessors, attributes(kirin))]
pub fn derive_has_successors(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_field_iter!(&ast, "successors", Block, HasSuccessors).into()
}

#[proc_macro_derive(HasSuccessorsMut, attributes(kirin))]
pub fn derive_has_successors_mut(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_field_iter_mut!(&ast, "successors_mut", Block, HasSuccessorsMut).into()
}

#[proc_macro_derive(HasRegions, attributes(kirin))]
pub fn derive_has_regions(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_field_iter!(&ast, "regions", Region, HasRegions).into()
}

#[proc_macro_derive(HasRegionsMut, attributes(kirin))]
pub fn derive_has_regions_mut(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_field_iter_mut!(&ast, "regions_mut", Region, HasRegionsMut).into()
}

#[proc_macro_derive(IsTerminator, attributes(kirin))]
pub fn derive_is_terminator(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_check!(&ast, is_terminator, IsTerminator).into()
}

#[proc_macro_derive(IsConstant, attributes(kirin))]
pub fn derive_is_constant(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_check!(&ast, is_constant, IsConstant).into()
}

#[proc_macro_derive(IsPure, attributes(kirin))]
pub fn derive_is_pure(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_check!(&ast, is_pure, IsPure).into()
}
