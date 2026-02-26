extern crate proc_macro;

use kirin_derive_core::prelude::darling;
use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse_macro_input;

use kirin_derive_dialect::{
    builder::DeriveBuilder,
    field::{DeriveFieldIter, FieldIterKind},
    marker,
    property::{DeriveProperty, PropertyKind},
};

const DEFAULT_IR_CRATE: &str = "::kirin::ir";
const TRAIT_LIFETIME: &str = "'a";

#[derive(Clone, Copy)]
struct FieldIterConfig {
    kind: FieldIterKind,
    mutable: bool,
    trait_name: &'static str,
    matching_type: &'static str,
    trait_method: &'static str,
    trait_type_iter: &'static str,
}

impl FieldIterConfig {
    const fn new(
        kind: FieldIterKind,
        mutable: bool,
        trait_name: &'static str,
        matching_type: &'static str,
        trait_method: &'static str,
        trait_type_iter: &'static str,
    ) -> Self {
        Self {
            kind,
            mutable,
            trait_name,
            matching_type,
            trait_method,
            trait_type_iter,
        }
    }
}

#[derive(Clone, Copy)]
struct PropertyConfig {
    kind: PropertyKind,
    trait_name: &'static str,
    trait_method: &'static str,
}

impl PropertyConfig {
    const fn new(kind: PropertyKind, trait_name: &'static str, trait_method: &'static str) -> Self {
        Self {
            kind,
            trait_name,
            trait_method,
        }
    }
}

const HAS_ARGUMENTS: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Arguments,
    false,
    "HasArguments",
    "SSAValue",
    "arguments",
    "Iter",
);
const HAS_ARGUMENTS_MUT: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Arguments,
    true,
    "HasArgumentsMut",
    "SSAValue",
    "arguments_mut",
    "IterMut",
);
const HAS_RESULTS: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Results,
    false,
    "HasResults",
    "ResultValue",
    "results",
    "Iter",
);
const HAS_RESULTS_MUT: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Results,
    true,
    "HasResultsMut",
    "ResultValue",
    "results_mut",
    "IterMut",
);
const HAS_BLOCKS: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Blocks,
    false,
    "HasBlocks",
    "Block",
    "blocks",
    "Iter",
);
const HAS_BLOCKS_MUT: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Blocks,
    true,
    "HasBlocksMut",
    "Block",
    "blocks_mut",
    "IterMut",
);
const HAS_SUCCESSORS: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Successors,
    false,
    "HasSuccessors",
    "Successor",
    "successors",
    "Iter",
);
const HAS_SUCCESSORS_MUT: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Successors,
    true,
    "HasSuccessorsMut",
    "Successor",
    "successors_mut",
    "IterMut",
);
const HAS_REGIONS: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Regions,
    false,
    "HasRegions",
    "Region",
    "regions",
    "Iter",
);
const HAS_REGIONS_MUT: FieldIterConfig = FieldIterConfig::new(
    FieldIterKind::Regions,
    true,
    "HasRegionsMut",
    "Region",
    "regions_mut",
    "IterMut",
);

const FIELD_ITER_CONFIGS: [FieldIterConfig; 10] = [
    HAS_ARGUMENTS,
    HAS_ARGUMENTS_MUT,
    HAS_RESULTS,
    HAS_RESULTS_MUT,
    HAS_BLOCKS,
    HAS_BLOCKS_MUT,
    HAS_SUCCESSORS,
    HAS_SUCCESSORS_MUT,
    HAS_REGIONS,
    HAS_REGIONS_MUT,
];

const IS_TERMINATOR: PropertyConfig =
    PropertyConfig::new(PropertyKind::Terminator, "IsTerminator", "is_terminator");
const IS_CONSTANT: PropertyConfig =
    PropertyConfig::new(PropertyKind::Constant, "IsConstant", "is_constant");
const IS_PURE: PropertyConfig = PropertyConfig::new(PropertyKind::Pure, "IsPure", "is_pure");
const IS_SPECULATABLE: PropertyConfig = PropertyConfig::new(
    PropertyKind::Speculatable,
    "IsSpeculatable",
    "is_speculatable",
);

const PROPERTY_CONFIGS: [PropertyConfig; 4] =
    [IS_TERMINATOR, IS_CONSTANT, IS_PURE, IS_SPECULATABLE];

fn emit_field_iter(
    ast: &syn::DeriveInput,
    config: FieldIterConfig,
) -> darling::Result<proc_macro2::TokenStream> {
    new_field_iter(config).emit(ast)
}

fn emit_property(
    ast: &syn::DeriveInput,
    config: PropertyConfig,
) -> darling::Result<proc_macro2::TokenStream> {
    new_property(config).emit(ast)
}

fn new_field_iter(config: FieldIterConfig) -> DeriveFieldIter {
    DeriveFieldIter::new(
        config.kind,
        config.mutable,
        DEFAULT_IR_CRATE,
        config.trait_name,
        config.matching_type,
        config.trait_method,
        config.trait_type_iter,
    )
    .with_trait_lifetime(TRAIT_LIFETIME)
}

fn new_property(config: PropertyConfig) -> DeriveProperty {
    DeriveProperty::new(
        config.kind,
        DEFAULT_IR_CRATE,
        config.trait_name,
        config.trait_method,
        "bool",
    )
}

#[proc_macro_derive(Dialect, attributes(kirin, wraps))]
pub fn derive_statement(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input =
        kirin_derive_core::ir::Input::<kirin_derive_core::ir::StandardLayout>::from_derive_input(
            &ast,
        );

    let ir = match ir_input {
        Ok(ir) => ir,
        Err(e) => return e.write_errors().into(),
    };

    let mut tokens = proc_macro2::TokenStream::new();

    for config in FIELD_ITER_CONFIGS {
        match new_field_iter(config).emit_from_input(&ir) {
            Ok(t) => tokens.extend(t),
            Err(e) => tokens.extend(e.write_errors()),
        }
    }

    for config in PROPERTY_CONFIGS {
        match new_property(config).emit_from_input(&ir) {
            Ok(t) => tokens.extend(t),
            Err(e) => tokens.extend(e.write_errors()),
        }
    }

    match DeriveBuilder::default().emit_from_input(&ir) {
        Ok(t) => tokens.extend(t),
        Err(e) => tokens.extend(e.write_errors()),
    }

    let default_crate: syn::Path = syn::parse_quote!(::kirin::ir);
    let crate_path = ir.attrs.crate_path.as_ref().unwrap_or(&default_crate);
    let trait_path: syn::Path = syn::parse_quote!(#crate_path::Dialect);
    marker::derive_marker(&ir, &trait_path).to_tokens(&mut tokens);

    tokens.into()
}

fn do_derive_field_iter(input: TokenStream, config: FieldIterConfig) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match emit_field_iter(&ast, config) {
        Ok(t) => t.into(),
        Err(e) => e.write_errors().into(),
    }
}

fn do_derive_property(input: TokenStream, config: PropertyConfig) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match emit_property(&ast, config) {
        Ok(t) => t.into(),
        Err(e) => e.write_errors().into(),
    }
}

macro_rules! derive_field_iter_macro {
    ($fn_name:ident, $trait_name:ident, $config:ident) => {
        #[proc_macro_derive($trait_name, attributes(kirin, wraps))]
        pub fn $fn_name(input: TokenStream) -> TokenStream {
            do_derive_field_iter(input, $config)
        }
    };
}

macro_rules! derive_property_macro {
    ($fn_name:ident, $trait_name:ident, $config:ident) => {
        #[proc_macro_derive($trait_name, attributes(kirin, wraps))]
        pub fn $fn_name(input: TokenStream) -> TokenStream {
            do_derive_property(input, $config)
        }
    };
}

derive_field_iter_macro!(derive_has_arguments, HasArguments, HAS_ARGUMENTS);
derive_field_iter_macro!(derive_has_arguments_mut, HasArgumentsMut, HAS_ARGUMENTS_MUT);
derive_field_iter_macro!(derive_has_results, HasResults, HAS_RESULTS);
derive_field_iter_macro!(derive_has_results_mut, HasResultsMut, HAS_RESULTS_MUT);
derive_field_iter_macro!(derive_has_blocks, HasBlocks, HAS_BLOCKS);
derive_field_iter_macro!(derive_has_blocks_mut, HasBlocksMut, HAS_BLOCKS_MUT);
derive_field_iter_macro!(derive_has_successors, HasSuccessors, HAS_SUCCESSORS);
derive_field_iter_macro!(
    derive_has_successors_mut,
    HasSuccessorsMut,
    HAS_SUCCESSORS_MUT
);
derive_field_iter_macro!(derive_has_regions, HasRegions, HAS_REGIONS);
derive_field_iter_macro!(derive_has_regions_mut, HasRegionsMut, HAS_REGIONS_MUT);

derive_property_macro!(derive_is_terminator, IsTerminator, IS_TERMINATOR);
derive_property_macro!(derive_is_constant, IsConstant, IS_CONSTANT);
derive_property_macro!(derive_is_pure, IsPure, IS_PURE);
derive_property_macro!(derive_is_speculatable, IsSpeculatable, IS_SPECULATABLE);

#[proc_macro_derive(StageMeta, attributes(stage))]
pub fn derive_stage_meta(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match kirin_derive_dialect::stage_info::generate(&ast) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
