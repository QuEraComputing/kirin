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

#[derive(Clone, Copy)]
pub(crate) struct LocalFieldIterConfig {
    kind: FieldIterKind,
    mutable: bool,
    trait_name: &'static str,
    matching_type: &'static str,
    trait_method: &'static str,
    trait_type_iter: &'static str,
}

impl LocalFieldIterConfig {
    pub(crate) const fn new(
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
pub(crate) struct LocalPropertyConfig {
    kind: PropertyKind,
    trait_name: &'static str,
    trait_method: &'static str,
}

impl LocalPropertyConfig {
    pub(crate) const fn new(
        kind: PropertyKind,
        trait_name: &'static str,
        trait_method: &'static str,
    ) -> Self {
        Self {
            kind,
            trait_name,
            trait_method,
        }
    }
}

pub(crate) const HAS_ARGUMENTS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Arguments,
    false,
    "HasArguments",
    "SSAValue",
    "arguments",
    "Iter",
);
pub(crate) const HAS_ARGUMENTS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Arguments,
    true,
    "HasArgumentsMut",
    "SSAValue",
    "arguments_mut",
    "IterMut",
);
pub(crate) const HAS_RESULTS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Results,
    false,
    "HasResults",
    "ResultValue",
    "results",
    "Iter",
);
pub(crate) const HAS_RESULTS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Results,
    true,
    "HasResultsMut",
    "ResultValue",
    "results_mut",
    "IterMut",
);
pub(crate) const HAS_BLOCKS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Blocks,
    false,
    "HasBlocks",
    "Block",
    "blocks",
    "Iter",
);
pub(crate) const HAS_BLOCKS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Blocks,
    true,
    "HasBlocksMut",
    "Block",
    "blocks_mut",
    "IterMut",
);
pub(crate) const HAS_SUCCESSORS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Successors,
    false,
    "HasSuccessors",
    "Successor",
    "successors",
    "Iter",
);
pub(crate) const HAS_SUCCESSORS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Successors,
    true,
    "HasSuccessorsMut",
    "Successor",
    "successors_mut",
    "IterMut",
);
pub(crate) const HAS_REGIONS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Regions,
    false,
    "HasRegions",
    "Region",
    "regions",
    "Iter",
);
pub(crate) const HAS_REGIONS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Regions,
    true,
    "HasRegionsMut",
    "Region",
    "regions_mut",
    "IterMut",
);
pub(crate) const HAS_DIGRAPHS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Digraphs,
    false,
    "HasDigraphs",
    "DiGraph",
    "digraphs",
    "Iter",
);
pub(crate) const HAS_DIGRAPHS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Digraphs,
    true,
    "HasDigraphsMut",
    "DiGraph",
    "digraphs_mut",
    "IterMut",
);
pub(crate) const HAS_UNGRAPHS: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Ungraphs,
    false,
    "HasUngraphs",
    "UnGraph",
    "ungraphs",
    "Iter",
);
pub(crate) const HAS_UNGRAPHS_MUT: LocalFieldIterConfig = LocalFieldIterConfig::new(
    FieldIterKind::Ungraphs,
    true,
    "HasUngraphsMut",
    "UnGraph",
    "ungraphs_mut",
    "IterMut",
);

pub(crate) const FIELD_ITER_CONFIGS: [LocalFieldIterConfig; 14] = [
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

pub(crate) const IS_TERMINATOR: LocalPropertyConfig =
    LocalPropertyConfig::new(PropertyKind::Terminator, "IsTerminator", "is_terminator");
pub(crate) const IS_CONSTANT: LocalPropertyConfig =
    LocalPropertyConfig::new(PropertyKind::Constant, "IsConstant", "is_constant");
pub(crate) const IS_PURE: LocalPropertyConfig =
    LocalPropertyConfig::new(PropertyKind::Pure, "IsPure", "is_pure");
pub(crate) const IS_SPECULATABLE: LocalPropertyConfig = LocalPropertyConfig::new(
    PropertyKind::Speculatable,
    "IsSpeculatable",
    "is_speculatable",
);

pub(crate) const IS_EDGE: LocalPropertyConfig =
    LocalPropertyConfig::new(PropertyKind::Edge, "IsEdge", "is_edge");

pub(crate) const PROPERTY_CONFIGS: [LocalPropertyConfig; 5] = [
    IS_TERMINATOR,
    IS_CONSTANT,
    IS_PURE,
    IS_SPECULATABLE,
    IS_EDGE,
];

pub(crate) fn to_field_iter_config(config: LocalFieldIterConfig) -> FieldIterConfig {
    FieldIterConfig {
        kind: config.kind,
        mutable: config.mutable,
        trait_name: config.trait_name,
        matching_type: config.matching_type,
        trait_method: config.trait_method,
        trait_type_iter: config.trait_type_iter,
    }
}

pub(crate) fn to_bool_property_config(config: LocalPropertyConfig) -> BoolPropertyConfig {
    BoolPropertyConfig {
        kind: config.kind,
        trait_name: config.trait_name,
        trait_method: config.trait_method,
    }
}

/// Generate the full Dialect derive output (all field iters, properties, builder, marker).
pub(crate) fn generate_dialect(ast: &syn::DeriveInput) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(ast)?;

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

    builder.build()
}

/// Generate a single field-iter derive.
pub(crate) fn generate_field_iter(
    ast: &syn::DeriveInput,
    config: LocalFieldIterConfig,
) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(ast)?;
    ir.compose()
        .add(TraitImplTemplate::field_iter(
            to_field_iter_config(config),
            DEFAULT_IR_CRATE,
            TRAIT_LIFETIME,
        ))
        .build()
}

/// Generate a single bool-property derive.
pub(crate) fn generate_property(
    ast: &syn::DeriveInput,
    config: LocalPropertyConfig,
) -> darling::Result<TokenStream> {
    let ir = Input::<StandardLayout>::from_derive_input(ast)?;
    ir.compose()
        .add(TraitImplTemplate::bool_property(
            to_bool_property_config(config),
            DEFAULT_IR_CRATE,
        ))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    fn generate_dialect_code(input: syn::DeriveInput) -> String {
        let tokens = generate_dialect(&input).expect("Failed to generate Dialect derive");
        rustfmt(tokens.to_string())
    }

    fn generate_stage_meta_code(input: syn::DeriveInput) -> String {
        let tokens = kirin_derive_toolkit::stage_info::generate(&input)
            .expect("Failed to generate StageMeta derive");
        rustfmt(tokens.to_string())
    }

    // ---- Dialect derive: struct with SSA fields ----

    #[test]
    fn test_dialect_derive_struct_with_ssa_fields() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct BinaryOp {
                result: SSAValue,
                lhs: Value,
                rhs: Value,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with Region and Block fields ----

    #[test]
    fn test_dialect_derive_struct_with_region_block() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct IfOp {
                condition: Value,
                then_block: Block,
                else_block: Block,
                body: Region,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with Successor fields ----

    #[test]
    fn test_dialect_derive_struct_with_successors() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct Branch {
                target: Successor,
                args: Value,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with terminator annotation ----

    #[test]
    fn test_dialect_derive_struct_terminator() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, terminator)]
            struct Return {
                value: Value,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with all property annotations ----

    #[test]
    fn test_dialect_derive_struct_all_properties() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, constant, pure, speculatable)]
            struct Constant {
                #[kirin(type = SimpleType::placeholder())]
                result: ResultValue,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: enum with #[wraps] variants ----

    #[test]
    fn test_dialect_derive_enum_with_wraps() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum ArithLanguage {
                #[wraps]
                Add(AddOp),
                #[wraps]
                Sub(SubOp),
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: enum with mixed wraps and terminator ----

    #[test]
    fn test_dialect_derive_enum_wraps_with_terminator() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum CfOps {
                #[wraps]
                Branch(BranchOp),
                #[wraps]
                #[kirin(terminator)]
                Return(ReturnOp),
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: custom crate path ----

    #[test]
    fn test_dialect_derive_custom_crate_path() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, crate = kirin_ir)]
            struct Nop {}
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Standalone IsTerminator derive ----

    #[test]
    fn test_standalone_is_terminator() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, terminator)]
            struct Return {
                value: Value,
            }
        };
        let tokens =
            generate_property(&input, IS_TERMINATOR).expect("Failed to generate IsTerminator");
        insta::assert_snapshot!(rustfmt(tokens.to_string()));
    }

    // ---- Standalone HasArguments derive ----

    #[test]
    fn test_standalone_has_arguments() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct BinaryOp {
                result: SSAValue,
                lhs: Value,
                rhs: Value,
            }
        };
        let tokens =
            generate_field_iter(&input, HAS_ARGUMENTS).expect("Failed to generate HasArguments");
        insta::assert_snapshot!(rustfmt(tokens.to_string()));
    }

    // ---- StageMeta derive: single dialect ----

    #[test]
    fn test_stage_meta_single_dialect() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            enum SimpleStage {
                #[stage(name = "arith")]
                Arith(StageInfo<ArithDialect>),
            }
        };
        insta::assert_snapshot!(generate_stage_meta_code(input));
    }

    // ---- StageMeta derive: multi dialect ----

    #[test]
    fn test_stage_meta_multi_dialect() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            enum CompositeStage {
                #[stage(name = "arith")]
                Arith(StageInfo<ArithDialect>),
                #[stage(name = "func")]
                Func(StageInfo<FuncDialect>),
                #[stage(name = "cf")]
                Cf(StageInfo<CfDialect>),
            }
        };
        insta::assert_snapshot!(generate_stage_meta_code(input));
    }

    // ---- StageMeta derive: duplicate dialect type ----

    #[test]
    fn test_stage_meta_duplicate_dialect() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            enum MultiArithStage {
                #[stage(name = "arith_opt")]
                ArithOpt(StageInfo<ArithDialect>),
                #[stage(name = "arith_lower")]
                ArithLower(StageInfo<ArithDialect>),
            }
        };
        insta::assert_snapshot!(generate_stage_meta_code(input));
    }

    // ---- Dialect derive: union should error ----

    #[test]
    fn test_dialect_derive_union_error() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            union MyUnion {
                x: i32,
                y: f32,
            }
        };
        let result = generate_dialect(&input);
        assert!(result.is_err(), "union should produce an error");
    }

    // ---- Dialect derive: struct with no fields (unit-like) ----

    #[test]
    fn test_dialect_derive_struct_no_fields() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct Nop {}
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with Vec<SSAValue> field ----

    #[test]
    fn test_dialect_derive_struct_vec_ssa_value() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct CallOp {
                args: Vec<SSAValue>,
                #[kirin(type = SimpleType::placeholder())]
                result: ResultValue,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with Option<Block> field ----

    #[test]
    fn test_dialect_derive_struct_option_block() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct ConditionalOp {
                cond: SSAValue,
                then_block: Block,
                else_block: Option<Block>,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with Symbol field ----

    #[test]
    fn test_dialect_derive_struct_symbol() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct CallExtern {
                target: Symbol,
                args: Vec<SSAValue>,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: enum with mixed wrapper and non-wrapper variants ----

    #[test]
    fn test_dialect_derive_enum_mixed_wraps_and_fields() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum MixedOps {
                #[wraps]
                Add(AddOp),
                Literal { value: i64 },
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Property validation: constant without pure should error ----

    #[test]
    fn test_is_constant_without_pure_error() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, constant)]
            struct BadConstant {
                value: i64,
            }
        };
        let result = generate_property(&input, IS_CONSTANT);
        assert!(result.is_err(), "constant without pure should error");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("pure"),
            "Error should mention pure requirement: {err}"
        );
    }

    // ---- Property validation: speculatable without pure should error ----

    #[test]
    fn test_is_speculatable_without_pure_error() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, speculatable)]
            struct BadSpec {
                value: i64,
            }
        };
        let result = generate_property(&input, IS_SPECULATABLE);
        assert!(result.is_err(), "speculatable without pure should error");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("pure"),
            "Error should mention pure requirement: {err}"
        );
    }

    // ---- Property validation: constant with pure should succeed ----

    #[test]
    fn test_is_constant_with_pure_succeeds() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, constant, pure)]
            struct GoodConstant {
                value: i64,
            }
        };
        let result = generate_property(&input, IS_CONSTANT);
        assert!(result.is_ok(), "constant with pure should succeed");
    }

    // ---- Enum variant: constant without pure on specific variant ----
    // NOTE: Design issue — BoolProperty::for_variant does not call validate(),
    // so per-variant constant-without-pure is not caught at derive time.
    // The validation only runs through the for_struct path (struct inputs).
    // This test documents the current behavior.

    #[test]
    fn test_enum_variant_constant_without_pure_errors() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum Ops {
                #[kirin(constant)]
                Lit { value: i64 },
                Add { lhs: SSAValue, rhs: SSAValue },
            }
        };
        // for_variant now validates: constant requires pure
        let result = generate_property(&input, IS_CONSTANT);
        assert!(
            result.is_err(),
            "constant without pure should error on enum variants too"
        );
    }

    // ---- Standalone HasResults derive ----

    #[test]
    fn test_standalone_has_results() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct UnaryOp {
                #[kirin(type = SimpleType::placeholder())]
                result: ResultValue,
                arg: SSAValue,
            }
        };
        let tokens =
            generate_field_iter(&input, HAS_RESULTS).expect("Failed to generate HasResults");
        insta::assert_snapshot!(rustfmt(tokens.to_string()));
    }

    // ---- Standalone HasRegions derive ----

    #[test]
    fn test_standalone_has_regions() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct Lambda {
                body: Region,
            }
        };
        let tokens =
            generate_field_iter(&input, HAS_REGIONS).expect("Failed to generate HasRegions");
        insta::assert_snapshot!(rustfmt(tokens.to_string()));
    }

    // ---- Dialect derive: struct with DiGraph field ----

    #[test]
    fn test_dialect_derive_struct_with_digraph() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct QuantumEval {
                qubit: SSAValue,
                angle: SSAValue,
                body: DiGraph,
                #[kirin(type = SimpleType::placeholder())]
                res: ResultValue,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with UnGraph field ----

    #[test]
    fn test_dialect_derive_struct_with_ungraph() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct ZxEval {
                boundary: Vec<SSAValue>,
                captures: Vec<SSAValue>,
                body: UnGraph,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Dialect derive: struct with edge attribute ----

    #[test]
    fn test_dialect_derive_struct_edge() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, edge)]
            struct ZxWire {
                #[kirin(type = SimpleType::placeholder())]
                res: ResultValue,
            }
        };
        insta::assert_snapshot!(generate_dialect_code(input));
    }

    // ---- Standalone HasDigraphs derive ----

    #[test]
    fn test_standalone_has_digraphs() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            struct QuantumEval {
                body: DiGraph,
            }
        };
        let tokens =
            generate_field_iter(&input, HAS_DIGRAPHS).expect("Failed to generate HasDigraphs");
        insta::assert_snapshot!(rustfmt(tokens.to_string()));
    }

    // ---- Standalone IsEdge derive ----

    #[test]
    fn test_standalone_is_edge() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType, edge)]
            struct Wire {
                #[kirin(type = SimpleType::placeholder())]
                res: ResultValue,
            }
        };
        let tokens = generate_property(&input, IS_EDGE).expect("Failed to generate IsEdge");
        insta::assert_snapshot!(rustfmt(tokens.to_string()));
    }

    // ---- StageMeta derive error: applied to struct ----

    #[test]
    fn test_stage_meta_on_struct_error() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            struct NotAnEnum {
                info: StageInfo<ArithDialect>,
            }
        };
        let result = kirin_derive_toolkit::stage_info::generate(&input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("enum"), "Expected enum-only error: {err}");
    }

    // ---- StageMeta derive error: empty enum ----

    #[test]
    fn test_stage_meta_empty_enum_error() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[stage(crate = "kirin_ir")]
            enum EmptyStage {}
        };
        let result = kirin_derive_toolkit::stage_info::generate(&input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("at least one"),
            "Expected at-least-one error: {err}"
        );
    }
}
