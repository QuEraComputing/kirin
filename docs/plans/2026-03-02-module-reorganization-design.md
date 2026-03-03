# Module Reorganization Design

**Date:** 2026-03-02
**Scope:** Break large monolithic files (200+ lines) into modular directories across 6 crates (15 files total)

## Principles

- `mod.rs` is only for `mod` declarations, `pub use` re-exports, and `#[cfg]` gating — no implementation
- Files named by type/trait name, not by concern
- No implementation changes — pure file reorganization
- All public APIs remain identical
- Follow AGENTS.md: split files over 200 lines; use `mod.rs` for multi-file modules

## Reorganizations

### 1. `kirin-ir/src/stage/` (new)

**Source:** `pipeline.rs` (stage traits) + `stage_dispatch/` (dispatch machinery)

```
stage/
├── mod.rs               # re-exports only
├── meta.rs              # HasStageInfo<L> + StageMeta traits + StageInfo<L> impls
├── action.rs            # StageAction, StageActionMut traits
├── dispatch.rs          # StageDispatch, StageDispatchMut, SupportsStageDispatch, SupportsStageDispatchMut + tuple impls
├── error.rs             # StageDispatchMiss, StageDispatchRequiredError
├── helpers.rs           # pub(super) dispatch_optional_with, dispatch_required_with, map_required_miss_or_else
├── pipeline_impl.rs     # impl Pipeline<S> dispatch methods
└── tests.rs
```

`pipeline.rs` shrinks to ~280 lines (Pipeline<S> struct + its own impls/tests). `stage_dispatch/` is removed.

### 2. `kirin-ir/src/node/function/` (from `node/function.rs`, 373 lines)

```
node/function/
├── mod.rs               # re-exports only
├── compile_stage.rs     # CompileStage identifier type
├── generic.rs           # Function, FunctionInfo (abstract/generic function)
├── staged.rs            # StagedFunction, StagedFunctionInfo<L>, StagedNamePolicy
└── specialized.rs       # SpecializedFunction, SpecializedFunctionInfo<L> + From conversions
```

### 3. `kirin-ir/src/signature/` (from `signature.rs`, 228 lines)

```
signature/
├── mod.rs               # re-exports only
├── signature.rs         # Signature<T, C> struct + Default impl
├── semantics.rs         # SignatureCmp, SignatureSemantics, ExactSemantics, LatticeSemantics
└── tests.rs
```

### 4. `kirin-chumsky/src/ast/` (from `ast.rs`, 479 lines)

```
ast/
├── mod.rs               # re-exports only
├── spanned.rs           # Spanned<T> wrapper + EmitIR impl
├── values.rs            # SSAValue, ResultValue, TypeofSSAValue, NameofSSAValue + EmitIR impls
├── blocks.rs            # Block, Region, BlockLabel, BlockHeader, BlockArgument + EmitIR impls + emit_block helper
└── symbols.rs           # SymbolName, FunctionType + EmitIR/PrettyPrint impls
```

### 5. `kirin-chumsky/src/parsers/` (from `parsers.rs`, 439 lines)

```
parsers/
├── mod.rs               # re-exports only
├── identifiers.rs       # identifier, any_identifier, symbol
├── values.rs            # ssa_name, ssa_value, result_value, nameof_ssa, typeof_ssa, literal_int, literal_float
├── blocks.rs            # block_label, block_argument, block_argument_list, block_header, block, region
└── function_type.rs     # function_type parser + StmtOutput type alias
```

### 6. `kirin-chumsky/src/traits/` (from `traits.rs`, 341 lines)

```
traits/
├── mod.rs               # re-exports + type aliases (ParserError, BoxedParser, RecursiveParser)
├── has_parser.rs        # TokenInput, HasParser, HasDialectParser, parse_ast, ParseError
├── emit_ir.rs           # EmitIR, DirectlyParsable, EmitContext
└── parse_text.rs        # ParseStatementText, ParseStatementTextExt, parse_statement_on_stage, collect_existing_ssas
```

### 7. `kirin-chumsky/src/builtins/` (from `builtins.rs`, 361 lines)

```
builtins/
├── mod.rs               # re-exports only
├── integer.rs           # signed_int_parser, unsigned_int_parser + i8..i64, u8..u64, isize, usize impls
├── float.rs             # float_parser + f32, f64 impls
└── primitive.rs         # bool, String impls
```

### 8. `kirin-chumsky-format/src/generate/ast/` (from `generate/ast.rs`, 1014 lines)

```
generate/ast/
├── mod.rs               # re-exports only
├── generate.rs          # GenerateAST struct + new() + generate() entry point
├── definition.rs        # generate_ast_definition, generate_struct_fields, generate_enum_variants, field_ast_type
├── trait_impls.rs       # generate_manual_struct_trait_impls, generate_manual_trait_impls_for_wrapper_enum, collect_value_types_needing_bounds
└── wrapper.rs           # generate_ast_self_wrapper
```

### 9. `kirin-chumsky-format/src/generate/emit_ir/` (from `generate/emit_ir.rs`, 810 lines)

```
generate/emit_ir/
├── mod.rs               # re-exports only
├── generate.rs          # GenerateEmitIR struct + new() + generate() + bounds helpers
├── struct_emit.rs       # generate_struct_emit, build_emit_components
├── enum_emit.rs         # generate_enum_emit, generate_variant_emit, generate_dialect_constructor_with_defaults
├── field_emit.rs        # generate_field_emit_calls
└── self_emit.rs         # generate_ast_self_emit_impl
```

### 10. `kirin-chumsky-format/src/generate/pretty_print/` (from `generate/pretty_print.rs`, 429 lines)

```
generate/pretty_print/
├── mod.rs               # re-exports only
├── generate.rs          # GeneratePrettyPrint struct + new() + generate() entry point
├── statement.rs         # generate_pretty_print for statements, field rendering logic
└── helpers.rs           # build_field_map, tokens_to_string_with_spacing
```

### 11. `kirin-chumsky-format/src/field_kind/` (from `field_kind.rs`, 399 lines)

```
field_kind/
├── mod.rs               # re-exports only
├── kind.rs              # FieldKind enum + all methods + collect_fields
└── scanner.rs           # ValueTypeScanner + fields_in_format
```

### 12. `kirin-chumsky-format/src/visitor/` (from `visitor.rs`, 343 lines)

```
visitor/
├── mod.rs               # re-exports only
├── format_visitor.rs    # FormatVisitor trait + visit_format fn + build_field_map helper
└── context.rs           # VisitorContext struct + impls
```

### 13. `kirin-derive-core/src/ir/statement/` (from `ir/statement.rs`, 649 lines)

```
ir/statement/
├── mod.rs               # re-exports only
├── definition.rs        # Statement<L> struct + new/from_derive_input/from_variant/update_fields/parse_field
├── accessors.rs         # field iteration/query: arguments(), results(), blocks(), field_bindings, collect_fields, etc.
└── tests.rs
```

### 14. `kirin-derive-core/src/codegen/` (from `codegen.rs`, 397 lines)

```
codegen/
├── mod.rs               # re-exports only
├── utils.rs             # combine_where_clauses, deduplicate_types, tuple_field_idents, renamed_field_idents
├── field_bindings.rs    # FieldBindings struct + impls
├── generics_builder.rs  # GenericsBuilder struct + impls
└── constructor.rs       # ConstructorBuilder struct + impls
```

### 15. `kirin-prettyless/src/document/` (from `document.rs`, 376 lines)

```
document/
├── mod.rs               # re-exports only
├── builder.rs           # Document<'a, L> struct + constructor + arena delegation methods
└── ir_render.rs         # IR printing methods (statement, block, region) + strip_trailing_whitespace
```

## What does NOT change

- All public APIs remain identical
- `lib.rs` re-exports stay the same
- Already-modularized directories (`builder/`, `abstract_interp/`, `generate/parser/`) are untouched
- No implementation logic is modified
