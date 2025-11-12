extern crate proc_macro;

use kirin_derive_core::prelude::*;
use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::parse_macro_input;

#[proc_macro_derive(Statement, attributes(kirin))]
pub fn derive_statement(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let arguments = derive_accessor!(&ast, "arguments", ::kirin_ir::SSAValue, ::kirin_ir::HasArguments);
    let results = derive_accessor!(&ast, "results", ::kirin_ir::ResultValue, ::kirin_ir::HasResults);
    let successors = derive_accessor!(&ast, "successors", ::kirin_ir::Block, ::kirin_ir::HasSuccessors);
    let regions = derive_accessor!(&ast, "regions", ::kirin_ir::Region, ::kirin_ir::HasRegions);
    let is_terminator = derive_check!(&ast, is_terminator, ::kirin_ir::IsTerminator);
    let is_constant = derive_check!(&ast, is_constant, ::kirin_ir::IsConstant);
    let is_pure = derive_check!(&ast, is_pure, ::kirin_ir::IsPure);

    let name = &ast.ident;
    let lifetime = syn::Lifetime::new("'a", Span::call_site());
    let mut trait_generics = ast.generics.clone();
    trait_generics
        .params
        .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
            lifetime.clone(),
        )));
    let (trait_impl_generics, _, _) = trait_generics.split_for_impl();
    let (_, input_ty_generics, input_where_clause) = ast.generics.split_for_impl();

    let generated = quote::quote! {
        #arguments
        #results
        #successors
        #regions
        #is_terminator
        #is_constant
        #is_pure

        impl #trait_impl_generics ::kirin_ir::Statement<#lifetime> for #name #input_ty_generics #input_where_clause {} // Use the extracted name here
    };
    generated.into()
}

#[proc_macro_derive(HasArguments, attributes(kirin))]
pub fn derive_has_arguments(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_accessor!(&ast, "arguments", ::kirin_ir::SSAValue, ::kirin_ir::HasArguments).into()
}

#[proc_macro_derive(HasResults, attributes(kirin))]
pub fn derive_has_results(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_accessor!(&ast, "results", ::kirin_ir::ResultValue, ::kirin_ir::HasResults).into()
}

#[proc_macro_derive(HasSuccessors, attributes(kirin))]
pub fn derive_has_successors(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_accessor!(&ast, "successors", ::kirin_ir::Block, ::kirin_ir::HasSuccessors).into()
}

#[proc_macro_derive(HasRegions, attributes(kirin))]
pub fn derive_has_regions(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_accessor!(&ast, "regions", ::kirin_ir::Region, ::kirin_ir::HasRegions).into()
}

#[proc_macro_derive(IsTerminator, attributes(kirin))]
pub fn derive_is_terminator(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_check!(&ast, is_terminator, ::kirin_ir::IsTerminator).into()
}

#[proc_macro_derive(IsConstant, attributes(kirin))]
pub fn derive_is_constant(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_check!(&ast, is_constant, ::kirin_ir::IsConstant).into()
}

#[proc_macro_derive(IsPure, attributes(kirin))]
pub fn derive_is_pure(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    derive_check!(&ast, is_pure, ::kirin_ir::IsPure).into()
}
