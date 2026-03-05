extern crate proc_macro;

use proc_macro::TokenStream;
use syn::parse_macro_input;

use kirin_derive_toolkit::ir::{Input, StandardLayout};
use kirin_derive_toolkit::template::{
    BuilderTemplate, TraitImplTemplate,
    method_pattern::bool_property::PropertyKind,
    method_pattern::field_collection::FieldIterKind,
    trait_impl::{BoolPropertyConfig, FieldIterConfig},
};

const DEFAULT_IR_CRATE: &str = "::kirin::ir";
const TRAIT_LIFETIME: &str = "'a";

#[derive(Clone, Copy)]
struct LocalFieldIterConfig {
    kind: FieldIterKind,
    mutable: bool,
    trait_name: &'static str,
    matching_type: &'static str,
    trait_method: &'static str,
    trait_type_iter: &'static str,
}

impl LocalFieldIterConfig {
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
struct LocalPropertyConfig {
    kind: PropertyKind,
    trait_name: &'static str,
    trait_method: &'static str,
}

impl LocalPropertyConfig {
    const fn new(kind: PropertyKind, trait_name: &'static str, trait_method: &'static str) -> Self {
        Self {
            kind,
            trait_name,
            trait_method,
        }
    }
}

const HAS_ARGUMENTS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Arguments,
    false,
    "HasArguments",
    "SSAValue",
    "arguments",
    "Iter",
);
const HAS_ARGUMENTS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Arguments,
    true,
    "HasArgumentsMut",
    "SSAValue",
    "arguments_mut",
    "IterMut",
);
const HAS_RESULTS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Results,
    false,
    "HasResults",
    "ResultValue",
    "results",
    "Iter",
);
const HAS_RESULTS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Results,
    true,
    "HasResultsMut",
    "ResultValue",
    "results_mut",
    "IterMut",
);
const HAS_BLOCKS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Blocks,
    false,
    "HasBlocks",
    "Block",
    "blocks",
    "Iter",
);
const HAS_BLOCKS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Blocks,
    true,
    "HasBlocksMut",
    "Block",
    "blocks_mut",
    "IterMut",
);
const HAS_SUCCESSORS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Successors,
    false,
    "HasSuccessors",
    "Successor",
    "successors",
    "Iter",
);
const HAS_SUCCESSORS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Successors,
    true,
    "HasSuccessorsMut",
    "Successor",
    "successors_mut",
    "IterMut",
);
const HAS_REGIONS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Regions,
    false,
    "HasRegions",
    "Region",
    "regions",
    "Iter",
);
const HAS_REGIONS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Regions,
    true,
    "HasRegionsMut",
    "Region",
    "regions_mut",
    "IterMut",
);

const FIELD_ITER_CONFIGS: [LocalFieldIterConfig; 10] = [
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

const IS_TERMINATOR: LocalPropertyConfig =
    LocalPropertyConfig::new(PropertyKind::Terminator, "IsTerminator", "is_terminator");
const IS_CONSTANT: LocalPropertyConfig =
    LocalPropertyConfig::new(PropertyKind::Constant, "IsConstant", "is_constant");
const IS_PURE: LocalPropertyConfig =
    LocalPropertyConfig::new(PropertyKind::Pure, "IsPure", "is_pure");
const IS_SPECULATABLE: LocalPropertyConfig = LocalPropertyConfig::new(
    PropertyKind::Speculatable,
    "IsSpeculatable",
    "is_speculatable",
);

const PROPERTY_CONFIGS: [LocalPropertyConfig; 4] =
    [IS_TERMINATOR, IS_CONSTANT, IS_PURE, IS_SPECULATABLE];

fn to_field_iter_config(config: LocalFieldIterConfig) -> FieldIterConfig {
    FieldIterConfig {
        kind: config.kind,
        mutable: config.mutable,
        trait_name: config.trait_name,
        matching_type: config.matching_type,
        trait_method: config.trait_method,
        trait_type_iter: config.trait_type_iter,
    }
}

fn to_bool_property_config(config: LocalPropertyConfig) -> BoolPropertyConfig {
    BoolPropertyConfig {
        kind: config.kind,
        trait_name: config.trait_name,
        trait_method: config.trait_method,
    }
}

#[proc_macro_derive(Dialect, attributes(kirin, wraps))]
pub fn derive_statement(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir = match Input::<StandardLayout>::from_derive_input(&ast) {
        Ok(ir) => ir,
        Err(e) => return e.write_errors().into(),
    };

    let default_crate: syn::Path = syn::parse_quote!(::kirin::ir);
    let crate_path = ir.attrs.crate_path.as_ref().unwrap_or(&default_crate);
    let trait_path: syn::Path = syn::parse_quote!(#crate_path::Dialect);

    let mut builder = ir.compose();

    for config in FIELD_ITER_CONFIGS {
        builder = builder.add(TraitImplTemplate::field_iter(
            to_field_iter_config(config),
            DEFAULT_IR_CRATE,
            TRAIT_LIFETIME,
        ));
    }

    for config in PROPERTY_CONFIGS {
        builder = builder.add(TraitImplTemplate::bool_property(
            to_bool_property_config(config),
            DEFAULT_IR_CRATE,
        ));
    }

    builder = builder
        .add(BuilderTemplate::new())
        .add(TraitImplTemplate::marker(&trait_path, &ir.attrs.ir_type));

    match builder.build() {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}

fn do_derive_field_iter(input: TokenStream, config: LocalFieldIterConfig) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let ir = match Input::<StandardLayout>::from_derive_input(&ast) {
        Ok(ir) => ir,
        Err(e) => return e.write_errors().into(),
    };
    match ir
        .compose()
        .add(TraitImplTemplate::field_iter(
            to_field_iter_config(config),
            DEFAULT_IR_CRATE,
            TRAIT_LIFETIME,
        ))
        .build()
    {
        Ok(t) => t.into(),
        Err(e) => e.write_errors().into(),
    }
}

fn do_derive_property(input: TokenStream, config: LocalPropertyConfig) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let ir = match Input::<StandardLayout>::from_derive_input(&ast) {
        Ok(ir) => ir,
        Err(e) => return e.write_errors().into(),
    };
    match ir
        .compose()
        .add(TraitImplTemplate::bool_property(
            to_bool_property_config(config),
            DEFAULT_IR_CRATE,
        ))
        .build()
    {
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
    match kirin_derive_toolkit::stage_info::generate(&ast) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
