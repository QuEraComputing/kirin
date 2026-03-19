use kirin_derive_toolkit::ir::{Input, StandardLayout};
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::template::{
    BuilderTemplate, TraitImplTemplate,
    method_pattern::bool_property::PropertyKind,
    method_pattern::field_collection::FieldIterKind,
    trait_impl::{BoolPropertyConfig, FieldIterConfig},
};
use proc_macro2::TokenStream;

const DEFAULT_IR_CRATE: &str = "::kirin::ir";
const TRAIT_LIFETIME: &str = "'a";

/// Type alias — previously a local wrapper; now that [`FieldIterConfig`] derives `Copy`,
/// we use the toolkit type directly.
pub(crate) type LocalFieldIterConfig = FieldIterConfig;

/// Type alias — previously a local wrapper; now that [`BoolPropertyConfig`] derives `Copy`,
/// we use the toolkit type directly.
pub(crate) type LocalPropertyConfig = BoolPropertyConfig;

pub(crate) const HAS_ARGUMENTS: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Arguments,
    mutable: false,
    trait_name: "HasArguments",
    matching_type: "SSAValue",
    trait_method: "arguments",
    trait_type_iter: "Iter",
};
pub(crate) const HAS_ARGUMENTS_MUT: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Arguments,
    mutable: true,
    trait_name: "HasArgumentsMut",
    matching_type: "SSAValue",
    trait_method: "arguments_mut",
    trait_type_iter: "IterMut",
};
pub(crate) const HAS_RESULTS: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Results,
    mutable: false,
    trait_name: "HasResults",
    matching_type: "ResultValue",
    trait_method: "results",
    trait_type_iter: "Iter",
};
pub(crate) const HAS_RESULTS_MUT: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Results,
    mutable: true,
    trait_name: "HasResultsMut",
    matching_type: "ResultValue",
    trait_method: "results_mut",
    trait_type_iter: "IterMut",
};
pub(crate) const HAS_BLOCKS: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Blocks,
    mutable: false,
    trait_name: "HasBlocks",
    matching_type: "Block",
    trait_method: "blocks",
    trait_type_iter: "Iter",
};
pub(crate) const HAS_BLOCKS_MUT: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Blocks,
    mutable: true,
    trait_name: "HasBlocksMut",
    matching_type: "Block",
    trait_method: "blocks_mut",
    trait_type_iter: "IterMut",
};
pub(crate) const HAS_SUCCESSORS: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Successors,
    mutable: false,
    trait_name: "HasSuccessors",
    matching_type: "Successor",
    trait_method: "successors",
    trait_type_iter: "Iter",
};
pub(crate) const HAS_SUCCESSORS_MUT: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Successors,
    mutable: true,
    trait_name: "HasSuccessorsMut",
    matching_type: "Successor",
    trait_method: "successors_mut",
    trait_type_iter: "IterMut",
};
pub(crate) const HAS_REGIONS: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Regions,
    mutable: false,
    trait_name: "HasRegions",
    matching_type: "Region",
    trait_method: "regions",
    trait_type_iter: "Iter",
};
pub(crate) const HAS_REGIONS_MUT: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Regions,
    mutable: true,
    trait_name: "HasRegionsMut",
    matching_type: "Region",
    trait_method: "regions_mut",
    trait_type_iter: "IterMut",
};
pub(crate) const HAS_DIGRAPHS: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Digraphs,
    mutable: false,
    trait_name: "HasDigraphs",
    matching_type: "DiGraph",
    trait_method: "digraphs",
    trait_type_iter: "Iter",
};
pub(crate) const HAS_DIGRAPHS_MUT: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Digraphs,
    mutable: true,
    trait_name: "HasDigraphsMut",
    matching_type: "DiGraph",
    trait_method: "digraphs_mut",
    trait_type_iter: "IterMut",
};
pub(crate) const HAS_UNGRAPHS: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Ungraphs,
    mutable: false,
    trait_name: "HasUngraphs",
    matching_type: "UnGraph",
    trait_method: "ungraphs",
    trait_type_iter: "Iter",
};
pub(crate) const HAS_UNGRAPHS_MUT: FieldIterConfig = FieldIterConfig {
    kind: FieldIterKind::Ungraphs,
    mutable: true,
    trait_name: "HasUngraphsMut",
    matching_type: "UnGraph",
    trait_method: "ungraphs_mut",
    trait_type_iter: "IterMut",
};

pub(crate) const FIELD_ITER_CONFIGS: [FieldIterConfig; 14] = [
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
    HAS_DIGRAPHS,
    HAS_DIGRAPHS_MUT,
    HAS_UNGRAPHS,
    HAS_UNGRAPHS_MUT,
];

pub(crate) const IS_TERMINATOR: BoolPropertyConfig = BoolPropertyConfig {
    kind: PropertyKind::Terminator,
    trait_name: "IsTerminator",
    trait_method: "is_terminator",
};
pub(crate) const IS_CONSTANT: BoolPropertyConfig = BoolPropertyConfig {
    kind: PropertyKind::Constant,
    trait_name: "IsConstant",
    trait_method: "is_constant",
};
pub(crate) const IS_PURE: BoolPropertyConfig = BoolPropertyConfig {
    kind: PropertyKind::Pure,
    trait_name: "IsPure",
    trait_method: "is_pure",
};
pub(crate) const IS_SPECULATABLE: BoolPropertyConfig = BoolPropertyConfig {
    kind: PropertyKind::Speculatable,
    trait_name: "IsSpeculatable",
    trait_method: "is_speculatable",
};
pub(crate) const IS_EDGE: BoolPropertyConfig = BoolPropertyConfig {
    kind: PropertyKind::Edge,
    trait_name: "IsEdge",
    trait_method: "is_edge",
};

pub(crate) const PROPERTY_CONFIGS: [BoolPropertyConfig; 5] = [
    IS_TERMINATOR,
    IS_CONSTANT,
    IS_PURE,
    IS_SPECULATABLE,
    IS_EDGE,
];

/// Generate the full Dialect derive output (all field iters, properties, builder, marker).
pub(crate) fn generate_dialect(ast: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(ast)?;

    let default_crate: syn::Path = syn::parse_quote!(::kirin::ir);
    let crate_path = ir.attrs.crate_path.as_ref().unwrap_or(&default_crate);
    let trait_path: syn::Path = syn::parse_quote!(#crate_path::Dialect);

    let mut builder = ir.compose();

    for config in FIELD_ITER_CONFIGS {
        builder = builder.add(TraitImplTemplate::field_iter(
            config,
            DEFAULT_IR_CRATE,
            TRAIT_LIFETIME,
        ));
    }

    for config in PROPERTY_CONFIGS {
        builder = builder.add(TraitImplTemplate::bool_property(config, DEFAULT_IR_CRATE));
    }

    builder = builder
        .add(BuilderTemplate::new())
        .add(TraitImplTemplate::marker(&trait_path, &ir.attrs.ir_type));

    builder.build()
}

/// Generate a single field-iter derive.
pub(crate) fn generate_field_iter(
    ast: &syn::DeriveInput,
    config: FieldIterConfig,
) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(ast)?;
    ir.compose()
        .add(TraitImplTemplate::field_iter(
            config,
            DEFAULT_IR_CRATE,
            TRAIT_LIFETIME,
        ))
        .build()
}

/// Generate a single bool-property derive.
pub(crate) fn generate_property(
    ast: &syn::DeriveInput,
    config: BoolPropertyConfig,
) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(ast)?;
    ir.compose()
        .add(TraitImplTemplate::bool_property(config, DEFAULT_IR_CRATE))
        .build()
}
