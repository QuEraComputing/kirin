extern crate proc_macro;

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse_macro_input;

use kirin_derive_dialect::{
    builder::DeriveBuilder,
    field::{DeriveFieldIter, FieldIterKind},
    marker,
    property::{DeriveProperty, PropertyKind},
};

#[proc_macro_derive(Dialect, attributes(kirin, wraps))]
pub fn derive_statement(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let mut tokens = proc_macro2::TokenStream::new();

    // FieldsIter
    let iter_configs = [
        (
            FieldIterKind::Arguments,
            false,
            "HasArguments",
            "SSAValue",
            "arguments",
            "Iter",
        ),
        (
            FieldIterKind::Arguments,
            true,
            "HasArgumentsMut",
            "SSAValue",
            "arguments_mut",
            "IterMut",
        ),
        (
            FieldIterKind::Results,
            false,
            "HasResults",
            "ResultValue",
            "results",
            "Iter",
        ),
        (
            FieldIterKind::Results,
            true,
            "HasResultsMut",
            "ResultValue",
            "results_mut",
            "IterMut",
        ),
        (
            FieldIterKind::Blocks,
            false,
            "HasBlocks",
            "Block",
            "blocks",
            "Iter",
        ),
        (
            FieldIterKind::Blocks,
            true,
            "HasBlocksMut",
            "Block",
            "blocks_mut",
            "IterMut",
        ),
        (
            FieldIterKind::Successors,
            false,
            "HasSuccessors",
            "Successor",
            "successors",
            "Iter",
        ),
        (
            FieldIterKind::Successors,
            true,
            "HasSuccessorsMut",
            "Successor",
            "successors_mut",
            "IterMut",
        ),
        (
            FieldIterKind::Regions,
            false,
            "HasRegions",
            "Region",
            "regions",
            "Iter",
        ),
        (
            FieldIterKind::Regions,
            true,
            "HasRegionsMut",
            "Region",
            "regions_mut",
            "IterMut",
        ),
    ];

    for (kind, mutable, trait_name, matching_type, trait_method, trait_type_iter) in iter_configs {
        let res = DeriveFieldIter::new(
            kind,
            mutable,
            "::kirin::ir",
            trait_name,
            matching_type,
            trait_method,
            trait_type_iter,
        )
        .with_trait_lifetime("'a")
        .emit(&ast);

        match res {
            Ok(t) => tokens.extend(t),
            Err(e) => tokens.extend(e.write_errors()),
        }
    }

    // Properties
    let props = [
        (PropertyKind::Terminator, "IsTerminator", "is_terminator"),
        (PropertyKind::Constant, "IsConstant", "is_constant"),
        (PropertyKind::Pure, "IsPure", "is_pure"),
    ];

    for (kind, trait_path, trait_method) in props {
        let res =
            DeriveProperty::new(kind, "::kirin::ir", trait_path, trait_method, "bool").emit(&ast);
        match res {
            Ok(t) => tokens.extend(t),
            Err(e) => tokens.extend(e.write_errors()),
        }
    }

    // Builder
    match DeriveBuilder::default().emit(&ast) {
        Ok(t) => tokens.extend(t),
        Err(e) => tokens.extend(e.write_errors()),
    }

    // Marker
    let ir_input = kirin_derive_core_2::ir::Input::<
        kirin_derive_core_2::ir::StandardLayout,
    >::from_derive_input(&ast);

    match ir_input {
        Ok(ir) => {
            let default_crate: syn::Path = syn::parse_quote!(::kirin::ir);
            let crate_path = ir.attrs.crate_path.as_ref().unwrap_or(&default_crate);
            let trait_path: syn::Path = syn::parse_quote!(#crate_path::Dialect);
            marker::derive_marker(&ir, &trait_path).to_tokens(&mut tokens);
        }
        Err(e) => tokens.extend(e.write_errors()),
    }

    tokens.into()
}

fn do_derive_field_iter(
    input: TokenStream,
    kind: FieldIterKind,
    mutable: bool,
    trait_name: &str,
    matching_type: &str,
    trait_method: &str,
    trait_type_iter: &str,
) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let res = DeriveFieldIter::new(
        kind,
        mutable,
        "::kirin::ir",
        trait_name,
        matching_type,
        trait_method,
        trait_type_iter,
    )
    .with_trait_lifetime("'a")
    .emit(&ast);
    match res {
        Ok(t) => t.into(),
        Err(e) => e.write_errors().into(),
    }
}

#[proc_macro_derive(HasArguments, attributes(kirin, wraps))]
pub fn derive_has_arguments(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Arguments,
        false,
        "HasArguments",
        "SSAValue",
        "arguments",
        "Iter",
    )
}

#[proc_macro_derive(HasArgumentsMut, attributes(kirin, wraps))]
pub fn derive_has_arguments_mut(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Arguments,
        true,
        "HasArgumentsMut",
        "SSAValue",
        "arguments_mut",
        "IterMut",
    )
}

#[proc_macro_derive(HasResults, attributes(kirin, wraps))]
pub fn derive_has_results(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Results,
        false,
        "HasResults",
        "ResultValue",
        "results",
        "Iter",
    )
}

#[proc_macro_derive(HasResultsMut, attributes(kirin, wraps))]
pub fn derive_has_results_mut(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Results,
        true,
        "HasResultsMut",
        "ResultValue",
        "results_mut",
        "IterMut",
    )
}

#[proc_macro_derive(HasBlocks, attributes(kirin, wraps))]
pub fn derive_has_blocks(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Blocks,
        false,
        "HasBlocks",
        "Block",
        "blocks",
        "Iter",
    )
}

#[proc_macro_derive(HasBlocksMut, attributes(kirin, wraps))]
pub fn derive_has_blocks_mut(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Blocks,
        true,
        "HasBlocksMut",
        "Block",
        "blocks_mut",
        "IterMut",
    )
}

#[proc_macro_derive(HasSuccessors, attributes(kirin, wraps))]
pub fn derive_has_successors(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Successors,
        false,
        "HasSuccessors",
        "Successor",
        "successors",
        "Iter",
    )
}

#[proc_macro_derive(HasSuccessorsMut, attributes(kirin, wraps))]
pub fn derive_has_successors_mut(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Successors,
        true,
        "HasSuccessorsMut",
        "Successor",
        "successors_mut",
        "IterMut",
    )
}

#[proc_macro_derive(HasRegions, attributes(kirin, wraps))]
pub fn derive_has_regions(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Regions,
        false,
        "HasRegions",
        "Region",
        "regions",
        "Iter",
    )
}

#[proc_macro_derive(HasRegionsMut, attributes(kirin, wraps))]
pub fn derive_has_regions_mut(input: TokenStream) -> TokenStream {
    do_derive_field_iter(
        input,
        FieldIterKind::Regions,
        true,
        "HasRegionsMut",
        "Region",
        "regions_mut",
        "IterMut",
    )
}

fn do_derive_property(
    input: TokenStream,
    kind: PropertyKind,
    trait_name: &str,
    trait_method: &str,
) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let res = DeriveProperty::new(kind, "::kirin::ir", trait_name, trait_method, "bool").emit(&ast);
    match res {
        Ok(t) => t.into(),
        Err(e) => e.write_errors().into(),
    }
}

#[proc_macro_derive(IsTerminator, attributes(kirin, wraps))]
pub fn derive_is_terminator(input: TokenStream) -> TokenStream {
    do_derive_property(
        input,
        PropertyKind::Terminator,
        "IsTerminator",
        "is_terminator",
    )
}

#[proc_macro_derive(IsConstant, attributes(kirin, wraps))]
pub fn derive_is_constant(input: TokenStream) -> TokenStream {
    do_derive_property(input, PropertyKind::Constant, "IsConstant", "is_constant")
}

#[proc_macro_derive(IsPure, attributes(kirin, wraps))]
pub fn derive_is_pure(input: TokenStream) -> TokenStream {
    do_derive_property(input, PropertyKind::Pure, "IsPure", "is_pure")
}
