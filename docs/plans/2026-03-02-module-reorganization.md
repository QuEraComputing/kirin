# Module Reorganization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split 15 large monolithic source files (200+ lines) into modular directories across 6 crates, without changing any implementation logic.

**Architecture:** Each file `foo.rs` becomes `foo/mod.rs` + submodules named by type/trait. `mod.rs` contains only `mod` declarations, `pub use` re-exports, and `#[cfg]` gating — zero implementation. All public APIs remain identical.

**Tech Stack:** Rust (edition 2024), cargo workspace with nextest

---

## Validation Pattern

Every task follows this pattern:
1. Create the new directory and files (move code, write `mod.rs`)
2. Delete the old file
3. Run `cargo build -p <crate>` to verify compilation
4. Run `cargo nextest run -p <crate>` to verify tests pass
5. Commit

---

### Task 1: `kirin-ir` — Create `stage/` module

**Files:**
- Delete: `crates/kirin-ir/src/stage_dispatch/` (the uncommitted directory)
- Create: `crates/kirin-ir/src/stage/mod.rs`
- Create: `crates/kirin-ir/src/stage/meta.rs`
- Create: `crates/kirin-ir/src/stage/action.rs`
- Create: `crates/kirin-ir/src/stage/dispatch.rs`
- Create: `crates/kirin-ir/src/stage/error.rs`
- Create: `crates/kirin-ir/src/stage/helpers.rs`
- Create: `crates/kirin-ir/src/stage/pipeline_impl.rs`
- Create: `crates/kirin-ir/src/stage/tests.rs`
- Modify: `crates/kirin-ir/src/pipeline.rs` — remove lines 14-122 (HasStageInfo + StageMeta)
- Modify: `crates/kirin-ir/src/lib.rs` — replace `mod stage_dispatch` with `mod stage`, update re-exports

**Step 1: Create `stage/` directory and files**

Move from `pipeline.rs` lines 14-58 (HasStageInfo trait + StageInfo impl) and lines 60-122 (StageMeta trait + StageInfo impl) into `stage/meta.rs`. Fix imports to use `crate::` paths.

Move from `stage_dispatch/core.rs`:
- Lines 5-18 (StageDispatchMiss, StageDispatchRequiredError) → `stage/error.rs`
- Lines 22-57 (StageAction, StageActionMut) → `stage/action.rs`
- Lines 59-173 (StageDispatch, StageDispatchMut, SupportsStageDispatch, SupportsStageDispatchMut + tuple impls) → `stage/dispatch.rs`

Move `stage_dispatch/helpers.rs` → `stage/helpers.rs` (change `super::` to `super::error::` etc.)

Move `stage_dispatch/pipeline_impl.rs` → `stage/pipeline_impl.rs` (update imports)

Move `stage_dispatch/tests.rs` → `stage/tests.rs` (update imports)

Write `stage/mod.rs`:
```rust
mod action;
mod dispatch;
mod error;
mod helpers;
mod meta;
mod pipeline_impl;

pub use action::{StageAction, StageActionMut};
pub use dispatch::{
    StageDispatch, StageDispatchMut, SupportsStageDispatch, SupportsStageDispatchMut,
};
pub use error::{StageDispatchMiss, StageDispatchRequiredError};
pub use meta::{HasStageInfo, StageMeta};

#[cfg(test)]
mod tests;
```

**Step 2: Update `pipeline.rs`**

Remove lines 14-122 (everything before `pub struct Pipeline<S>`). The file should start with imports and then `pub struct Pipeline<S>`. Update imports: add `use crate::stage::{HasStageInfo, StageMeta};` if needed by the remaining Pipeline impl blocks.

**Step 3: Update `lib.rs`**

Replace:
```rust
mod stage_dispatch;
```
with:
```rust
mod stage;
```

Replace:
```rust
pub use pipeline::{HasStageInfo, Pipeline, StageMeta};
pub use stage_dispatch::{
    StageAction, StageActionMut, StageDispatch, StageDispatchMiss, StageDispatchMut,
    StageDispatchRequiredError, SupportsStageDispatch, SupportsStageDispatchMut,
};
```
with:
```rust
pub use pipeline::Pipeline;
pub use stage::{
    HasStageInfo, StageMeta,
    StageAction, StageActionMut, StageDispatch, StageDispatchMiss, StageDispatchMut,
    StageDispatchRequiredError, SupportsStageDispatch, SupportsStageDispatchMut,
};
```

**Step 4: Delete old `stage_dispatch/` directory**

Remove `crates/kirin-ir/src/stage_dispatch/` entirely.

**Step 5: Verify**

Run: `cargo build -p kirin-ir`
Run: `cargo nextest run -p kirin-ir`
Expected: All pass

**Step 6: Commit**

```
refactor(ir): extract stage/ module from pipeline.rs and stage_dispatch/
```

---

### Task 2: `kirin-ir` — Split `node/function.rs` into `node/function/`

**Files:**
- Delete: `crates/kirin-ir/src/node/function.rs`
- Create: `crates/kirin-ir/src/node/function/mod.rs`
- Create: `crates/kirin-ir/src/node/function/compile_stage.rs`
- Create: `crates/kirin-ir/src/node/function/generic.rs`
- Create: `crates/kirin-ir/src/node/function/staged.rs`
- Create: `crates/kirin-ir/src/node/function/specialized.rs`

**Step 1: Create files**

`compile_stage.rs` — lines 39-52 from function.rs: `CompileStage` identifier macro + `impl CompileStage`. Needs `use crate::{identifier, arena::Id};`.

`generic.rs` — lines 54-65 (Function, StagedFunction identifiers), lines 92-134 (FunctionInfo struct + impl), line 289-293 (From<FunctionInfo> for Function). Needs `use indexmap::IndexMap; use crate::{identifier, arena::Id}; use super::compile_stage::CompileStage; use super::staged::StagedFunction; use crate::node::symbol::GlobalSymbol;`.

`staged.rs` — lines 67-79 (StagedNamePolicy), lines 136-245 (StagedFunctionInfo + impl), lines 295-299 (From<StagedFunctionInfo> for StagedFunction), lines 343-353 (GetInfo for StagedFunction). Needs imports for Dialect, Signature, SignatureCmp, SignatureSemantics, GlobalSymbol, SpecializedFunctionInfo, SpecializedFunction, Statement, GetInfo, Item, Arena.

`specialized.rs` — lines 81-89 (SpecializedFunction struct + impl), lines 247-287 (SpecializedFunctionInfo + bon builder), lines 301-305 (From<SpecializedFunctionInfo> for SpecializedFunction), lines 307-341 (SpecializedFunctionInfo methods), lines 355-373 (GetInfo for SpecializedFunction). Needs imports for Dialect, Signature, StagedFunction, Statement, GetInfo.

`mod.rs`:
```rust
mod compile_stage;
mod generic;
mod specialized;
mod staged;

pub use compile_stage::CompileStage;
pub use generic::{Function, FunctionInfo};
pub use specialized::{SpecializedFunction, SpecializedFunctionInfo};
pub use staged::{StagedFunction, StagedFunctionInfo, StagedNamePolicy};
```

**Important:** The module doc comment (lines 1-28) should go in `mod.rs` above the `mod` declarations.

**Step 2: Verify**

Run: `cargo build -p kirin-ir`
Run: `cargo nextest run -p kirin-ir`

**Step 3: Commit**

```
refactor(ir): split node/function.rs into node/function/ module
```

---

### Task 3: `kirin-ir` — Split `signature.rs` into `signature/`

**Files:**
- Delete: `crates/kirin-ir/src/signature.rs`
- Create: `crates/kirin-ir/src/signature/mod.rs`
- Create: `crates/kirin-ir/src/signature/signature.rs`
- Create: `crates/kirin-ir/src/signature/semantics.rs`
- Create: `crates/kirin-ir/src/signature/tests.rs`

**Step 1: Create files**

`signature.rs` — lines 1-25: `Signature<T, C>` struct + `Default` impl.

`semantics.rs` — lines 27-153: `SignatureCmp` enum, `SignatureSemantics` trait, `ExactSemantics`, `LatticeSemantics`. Needs `use super::signature::Signature; use crate::lattice::TypeLattice; use std::marker::PhantomData;`.

`tests.rs` — lines 155-228. Needs `use super::*;`.

`mod.rs`:
```rust
mod semantics;
mod signature;

pub use semantics::{ExactSemantics, LatticeSemantics, SignatureCmp, SignatureSemantics};
pub use signature::Signature;

#[cfg(test)]
mod tests;
```

**Step 2: Verify**

Run: `cargo build -p kirin-ir`
Run: `cargo nextest run -p kirin-ir`

**Step 3: Commit**

```
refactor(ir): split signature.rs into signature/ module
```

---

### Task 4: `kirin-chumsky` — Split `ast.rs` into `ast/`

**Files:**
- Delete: `crates/kirin-chumsky/src/ast.rs`
- Create: `crates/kirin-chumsky/src/ast/mod.rs`
- Create: `crates/kirin-chumsky/src/ast/spanned.rs`
- Create: `crates/kirin-chumsky/src/ast/values.rs`
- Create: `crates/kirin-chumsky/src/ast/blocks.rs`
- Create: `crates/kirin-chumsky/src/ast/symbols.rs`

**Step 1: Create files**

`spanned.rs` — lines 6 (use SimpleSpan), lines 12-46 (Spanned struct + Copy + PartialEq + Display + impl Spanned), lines 466-479 (EmitIR for Spanned). Needs appropriate imports.

`values.rs` — lines 48-102 (SSAValue, ResultValue, TypeofSSAValue, NameofSSAValue structs), lines 210-263 (EmitIR impls for SSAValue and ResultValue). Needs `use super::spanned::Spanned; use crate::traits::{EmitContext, EmitIR}; use kirin_ir::{Dialect, SSAKind};`.

`blocks.rs` — lines 116-189 (BlockLabel, BlockArgument, BlockHeader, Block, Region structs), lines 265-281 (EmitIR for BlockLabel), lines 309-464 (emit_block helper + EmitIR for Block + EmitIR for Region). Needs `use super::spanned::Spanned; use super::symbols::SymbolName; use crate::traits::{EmitContext, EmitIR}; use kirin_ir::{Dialect, GetInfo};`.

`symbols.rs` — lines 104-114 (SymbolName struct), lines 191-206 (FunctionType struct + PartialEq), lines 283-307 (EmitIR for SymbolName + PrettyPrint for SymbolName). Needs `use super::spanned::Spanned; use crate::traits::{EmitContext, EmitIR}; use kirin_ir::Dialect; use kirin_prettyless::{ArenaDoc, DocAllocator, Document, PrettyPrint};`.

`mod.rs`:
```rust
mod blocks;
mod spanned;
mod symbols;
mod values;

pub use blocks::{Block, BlockArgument, BlockHeader, BlockLabel, Region};
pub use spanned::Spanned;
pub use symbols::{FunctionType, SymbolName};
pub use values::{NameofSSAValue, ResultValue, SSAValue, TypeofSSAValue};
```

**Note:** `lib.rs` uses `pub mod ast;` and `pub use ast::*;` — this should still work since the `pub use` in mod.rs re-exports everything.

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky`
Run: `cargo nextest run -p kirin-chumsky`

**Step 3: Commit**

```
refactor(chumsky): split ast.rs into ast/ module
```

---

### Task 5: `kirin-chumsky` — Split `parsers.rs` into `parsers/`

**Files:**
- Delete: `crates/kirin-chumsky/src/parsers.rs`
- Create: `crates/kirin-chumsky/src/parsers/mod.rs`
- Create: `crates/kirin-chumsky/src/parsers/identifiers.rs`
- Create: `crates/kirin-chumsky/src/parsers/values.rs`
- Create: `crates/kirin-chumsky/src/parsers/blocks.rs`
- Create: `crates/kirin-chumsky/src/parsers/function_type.rs`

**Step 1: Create files**

`identifiers.rs` — lines 8-58: `identifier()`, `any_identifier()`, `symbol()`. Needs `use crate::ast::*; use crate::traits::*; use chumsky::prelude::*; use kirin_lexer::Token;`.

`values.rs` — lines 60-191: `ssa_name()`, `ssa_value()`, `result_value()`, `nameof_ssa()`, `typeof_ssa()`, `literal_int()`, `literal_float()`. Same imports.

`blocks.rs` — lines 193-379: `block_label()`, `StmtOutput` type alias, `block_argument()`, `block_argument_list()`, `block_header()`, `block()`, `region()`. Same imports.

`function_type.rs` — lines 381-439: `function_type()`. Same imports.

`mod.rs`:
```rust
mod blocks;
mod function_type;
mod identifiers;
mod values;

pub use blocks::{
    block, block_argument, block_argument_list, block_header, block_label, region, StmtOutput,
};
pub use function_type::function_type;
pub use identifiers::{any_identifier, identifier, symbol};
pub use values::{
    literal_float, literal_int, nameof_ssa, result_value, ssa_name, ssa_value, typeof_ssa,
};
```

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky`
Run: `cargo nextest run -p kirin-chumsky`

**Step 3: Commit**

```
refactor(chumsky): split parsers.rs into parsers/ module
```

---

### Task 6: `kirin-chumsky` — Split `traits.rs` into `traits/`

**Files:**
- Delete: `crates/kirin-chumsky/src/traits.rs`
- Create: `crates/kirin-chumsky/src/traits/mod.rs`
- Create: `crates/kirin-chumsky/src/traits/has_parser.rs`
- Create: `crates/kirin-chumsky/src/traits/emit_ir.rs`
- Create: `crates/kirin-chumsky/src/traits/parse_text.rs`

**Step 1: Create files**

`has_parser.rs` — lines 11-129: `TokenInput` trait + blanket impl, `HasParser`, `HasDialectParser`, `ParseError`, `parse_ast()`. Needs chumsky, kirin_lexer, kirin_ir imports. Also exports `ParserError`, `BoxedParser`, `RecursiveParser` type aliases (lines 22-31).

`emit_ir.rs` — lines 257-341: `EmitContext` struct + impl, `EmitIR` trait, `DirectlyParsable` trait + blanket impls for Vec/Option. Needs `use kirin_ir::{Dialect, StageInfo}; use rustc_hash::FxHashMap;`.

`parse_text.rs` — lines 131-255: `ParseStatementText`, `ParseStatementTextExt`, `collect_existing_ssas()`, `parse_statement_on_stage()`, StageInfo impl, Pipeline impl. Needs `use super::emit_ir::{EmitContext, EmitIR}; use super::has_parser::*; use kirin_ir::{Dialect, Pipeline, StageInfo};`.

`mod.rs`:
```rust
mod emit_ir;
mod has_parser;
mod parse_text;

pub use emit_ir::{DirectlyParsable, EmitContext, EmitIR};
pub use has_parser::{
    BoxedParser, HasDialectParser, HasParser, ParseError, ParserError, RecursiveParser,
    TokenInput, parse_ast,
};
pub use parse_text::{ParseStatementText, ParseStatementTextExt};
```

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky`
Run: `cargo nextest run -p kirin-chumsky`

**Step 3: Commit**

```
refactor(chumsky): split traits.rs into traits/ module
```

---

### Task 7: `kirin-chumsky` — Split `builtins.rs` into `builtins/`

**Files:**
- Delete: `crates/kirin-chumsky/src/builtins.rs`
- Create: `crates/kirin-chumsky/src/builtins/mod.rs`
- Create: `crates/kirin-chumsky/src/builtins/integer.rs`
- Create: `crates/kirin-chumsky/src/builtins/float.rs`
- Create: `crates/kirin-chumsky/src/builtins/primitive.rs`

**Step 1: Create files**

`integer.rs` — lines 22-226: `signed_int_parser()`, `unsigned_int_parser()`, all i8..isize impls, all u8..usize impls. Needs `use chumsky::prelude::*; use kirin_lexer::Token; use crate::traits::{BoxedParser, DirectlyParsable, HasParser, TokenInput};`.

`float.rs` — lines 68-256: `float_parser()`, f32 + f64 impls. Same imports.

`primitive.rs` — lines 262-308: bool + String impls. Same imports.

`mod.rs`:
```rust
mod float;
mod integer;
mod primitive;

#[cfg(test)]
mod tests;
```

**Important:** The tests module (lines 314-361) references `parse_with::<i32>`, `parse_with::<u32>`, etc. Move the tests to `builtins/tests.rs` with `use super::*; use crate::traits::HasParser;`.

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky`
Run: `cargo nextest run -p kirin-chumsky`

**Step 3: Commit**

```
refactor(chumsky): split builtins.rs into builtins/ module
```

---

### Task 8: `kirin-chumsky-format` — Split `generate/ast.rs` into `generate/ast/`

**Files:**
- Delete: `crates/kirin-chumsky-format/src/generate/ast.rs`
- Create: `crates/kirin-chumsky-format/src/generate/ast/mod.rs`
- Create: `crates/kirin-chumsky-format/src/generate/ast/generate.rs`
- Create: `crates/kirin-chumsky-format/src/generate/ast/definition.rs`
- Create: `crates/kirin-chumsky-format/src/generate/ast/trait_impls.rs`
- Create: `crates/kirin-chumsky-format/src/generate/ast/wrapper.rs`

**Step 1: Create files**

`generate.rs` — lines 24-98: `GenerateAST` struct, `new()`, `generate()`, `collect_value_types_needing_bounds()`.

`definition.rs` — lines 99-289 (`generate_ast_definition`), lines 653-766 (`generate_struct_fields`, `generate_enum_variants`, `field_ast_type`). These are all methods on `GenerateAST` — use `impl GenerateAST` blocks in each file referencing `use super::generate::GenerateAST;`.

`trait_impls.rs` — lines 290-459 (`generate_manual_struct_trait_impls`), lines 767-1014 (`generate_manual_trait_impls_for_wrapper_enum`).

`wrapper.rs` — lines 460-651 (`generate_ast_self_wrapper`).

`mod.rs`:
```rust
mod definition;
mod generate;
mod trait_impls;
mod wrapper;

pub use generate::GenerateAST;
```

**Note:** Since all functions are methods on `GenerateAST`, each file needs `impl GenerateAST { ... }` blocks. The methods called across files need `pub(super)` visibility.

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky-format`
Run: `cargo nextest run -p kirin-chumsky-format`

**Step 3: Commit**

```
refactor(chumsky-format): split generate/ast.rs into generate/ast/ module
```

---

### Task 9: `kirin-chumsky-format` — Split `generate/emit_ir.rs` into `generate/emit_ir/`

**Files:**
- Delete: `crates/kirin-chumsky-format/src/generate/emit_ir.rs`
- Create: `crates/kirin-chumsky-format/src/generate/emit_ir/mod.rs`
- Create: `crates/kirin-chumsky-format/src/generate/emit_ir/generate.rs`
- Create: `crates/kirin-chumsky-format/src/generate/emit_ir/struct_emit.rs`
- Create: `crates/kirin-chumsky-format/src/generate/emit_ir/enum_emit.rs`
- Create: `crates/kirin-chumsky-format/src/generate/emit_ir/field_emit.rs`
- Create: `crates/kirin-chumsky-format/src/generate/emit_ir/self_emit.rs`

**Step 1: Create files**

`generate.rs` — lines 22-177: `GenerateEmitIR` struct, `new()`, `generate()`, and all bounds/predicate helpers (`build_ast_ty_generics`, `language_output_emit_bound`, `ast_needs_language_output_emit_bound`, `statement_needs_language_output_emit_bound`, `statement_contains_statement_recursion_fields`, `ast_fields_contain_statement_recursion_fields`, `is_ir_type_a_type_param`).

`struct_emit.rs` — lines 448-569: `generate_struct_emit()`, `build_emit_components()`.

`enum_emit.rs` — lines 570-810: `generate_enum_emit()`, `generate_variant_emit()`, `generate_dialect_constructor_with_defaults()`.

`field_emit.rs` — lines 654-726: `generate_field_emit_calls()`.

`self_emit.rs` — lines 309-447: `generate_ast_self_emit_impl()`.

Note: `generate_emit_impl()` (lines 178-308) is the main dispatch that calls struct_emit and enum_emit — it should go in `generate.rs` alongside the entry point, or in its own file if `generate.rs` gets too large.

`mod.rs`:
```rust
mod enum_emit;
mod field_emit;
mod generate;
mod self_emit;
mod struct_emit;

pub use generate::GenerateEmitIR;
```

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky-format`
Run: `cargo nextest run -p kirin-chumsky-format`

**Step 3: Commit**

```
refactor(chumsky-format): split generate/emit_ir.rs into generate/emit_ir/ module
```

---

### Task 10: `kirin-chumsky-format` — Split `generate/pretty_print.rs` into `generate/pretty_print/`

**Files:**
- Delete: `crates/kirin-chumsky-format/src/generate/pretty_print.rs`
- Create: `crates/kirin-chumsky-format/src/generate/pretty_print/mod.rs`
- Create: `crates/kirin-chumsky-format/src/generate/pretty_print/generate.rs`
- Create: `crates/kirin-chumsky-format/src/generate/pretty_print/statement.rs`
- Create: `crates/kirin-chumsky-format/src/generate/pretty_print/helpers.rs`

**Step 1: Create files**

`generate.rs` — lines 21-94: `GeneratePrettyPrint` struct, `new()`, `generate()`, `ir_path()`.

`statement.rs` — lines 95-364: `generate_pretty_print()`, `generate_wrapper_struct_pretty_print()`, `generate_struct_print()`, `build_print_components()`, `generate_enum_print()`, `generate_variant_print()`, `generate_format_print()`.

`helpers.rs` — lines 365-429: `build_field_map()`, `tokens_to_string_with_spacing()`. These are module-level functions, not methods — so they can be `pub(super)`.

`mod.rs`:
```rust
mod generate;
mod helpers;
mod statement;

pub use generate::GeneratePrettyPrint;
```

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky-format`
Run: `cargo nextest run -p kirin-chumsky-format`

**Step 3: Commit**

```
refactor(chumsky-format): split generate/pretty_print.rs into generate/pretty_print/ module
```

---

### Task 11: `kirin-chumsky-format` — Split `field_kind.rs` into `field_kind/`

**Files:**
- Delete: `crates/kirin-chumsky-format/src/field_kind.rs`
- Create: `crates/kirin-chumsky-format/src/field_kind/mod.rs`
- Create: `crates/kirin-chumsky-format/src/field_kind/kind.rs`
- Create: `crates/kirin-chumsky-format/src/field_kind/scanner.rs`

**Step 1: Create files**

`kind.rs` — lines 1-295: `FieldKind` enum + all `impl FieldKind` methods + `collect_fields()` function.

`scanner.rs` — lines 296-399: `ValueTypeScanner` struct + impl + `Scan` impl + `fields_in_format()` function.

`mod.rs`:
```rust
mod kind;
mod scanner;

pub use kind::{FieldKind, collect_fields};
pub use scanner::{ValueTypeScanner, fields_in_format};
```

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky-format`
Run: `cargo nextest run -p kirin-chumsky-format`

**Step 3: Commit**

```
refactor(chumsky-format): split field_kind.rs into field_kind/ module
```

---

### Task 12: `kirin-chumsky-format` — Split `visitor.rs` into `visitor/`

**Files:**
- Delete: `crates/kirin-chumsky-format/src/visitor.rs`
- Create: `crates/kirin-chumsky-format/src/visitor/mod.rs`
- Create: `crates/kirin-chumsky-format/src/visitor/format_visitor.rs`
- Create: `crates/kirin-chumsky-format/src/visitor/context.rs`

**Step 1: Create files**

`format_visitor.rs` — lines 1-159: imports, `FormatVisitor` trait, `visit_format()` function, `build_field_map()` helper.

`context.rs` — lines 160-188: `VisitorContext` struct + impl.

Tests (lines 189-343) go in a separate `tests.rs` file in the `visitor/` directory, or can stay inline in `format_visitor.rs` since they test that module's code.

`mod.rs`:
```rust
mod context;
mod format_visitor;

pub use context::VisitorContext;
pub use format_visitor::{FormatVisitor, visit_format};

#[cfg(test)]
mod tests;
```

**Step 2: Verify**

Run: `cargo build -p kirin-chumsky-format`
Run: `cargo nextest run -p kirin-chumsky-format`

**Step 3: Commit**

```
refactor(chumsky-format): split visitor.rs into visitor/ module
```

---

### Task 13: `kirin-derive-core` — Split `ir/statement.rs` into `ir/statement/`

**Files:**
- Delete: `crates/kirin-derive-core/src/ir/statement.rs`
- Create: `crates/kirin-derive-core/src/ir/statement/mod.rs`
- Create: `crates/kirin-derive-core/src/ir/statement/definition.rs`
- Create: `crates/kirin-derive-core/src/ir/statement/accessors.rs`
- Create: `crates/kirin-derive-core/src/ir/statement/tests.rs`

**Step 1: Create files**

`definition.rs` — lines 1-204: `Statement<L>` struct definition + `new()`, `from_derive_input()`, `from_variant()`, `update_fields()`, `parse_field()`.

`accessors.rs` — lines 205-300: `iter_all_fields()`, `arguments()`, `results()`, `blocks()`, `successors()`, `regions()`, `values()`, `field_count()`, `named_field_idents()`, `is_tuple_style()`, `field_name_to_index()`, `field_bindings()`, `collect_fields()`. These are all `impl<L: Layout> Statement<L>` methods — use `use super::definition::Statement;` or just `use super::*;`.

`tests.rs` — lines 302-649.

`mod.rs`:
```rust
mod accessors;
mod definition;

pub use definition::Statement;

#[cfg(test)]
mod tests;
```

**Step 2: Verify**

Run: `cargo build -p kirin-derive-core`
Run: `cargo nextest run -p kirin-derive-core`

**Step 3: Commit**

```
refactor(derive-core): split ir/statement.rs into ir/statement/ module
```

---

### Task 14: `kirin-derive-core` — Split `codegen.rs` into `codegen/`

**Files:**
- Delete: `crates/kirin-derive-core/src/codegen.rs`
- Create: `crates/kirin-derive-core/src/codegen/mod.rs`
- Create: `crates/kirin-derive-core/src/codegen/utils.rs`
- Create: `crates/kirin-derive-core/src/codegen/field_bindings.rs`
- Create: `crates/kirin-derive-core/src/codegen/generics_builder.rs`
- Create: `crates/kirin-derive-core/src/codegen/constructor.rs`

**Step 1: Create files**

`utils.rs` — lines 12-21 (`tuple_field_idents`), lines 22-33 (`renamed_field_idents`), lines 117-147 (`combine_where_clauses`), lines 148-163 (`deduplicate_types`). Note: `tuple_field_idents` and `renamed_field_idents` are used by `FieldBindings` — make them `pub(super)` or `pub(crate)`.

`field_bindings.rs` — lines 34-116: `FieldBindings` struct + impl. Needs `use super::utils::{tuple_field_idents, renamed_field_idents};`.

`generics_builder.rs` — lines 164-277: `GenericsBuilder` struct + impl.

`constructor.rs` — lines 278-397: `ConstructorBuilder` struct + impl.

`mod.rs`:
```rust
mod constructor;
mod field_bindings;
mod generics_builder;
mod utils;

pub use constructor::ConstructorBuilder;
pub use field_bindings::FieldBindings;
pub use generics_builder::GenericsBuilder;
pub use utils::{combine_where_clauses, deduplicate_types};
```

**Step 2: Verify**

Run: `cargo build -p kirin-derive-core`
Run: `cargo nextest run -p kirin-derive-core`

**Step 3: Commit**

```
refactor(derive-core): split codegen.rs into codegen/ module
```

---

### Task 15: `kirin-prettyless` — Split `document.rs` into `document/`

**Files:**
- Delete: `crates/kirin-prettyless/src/document.rs`
- Create: `crates/kirin-prettyless/src/document/mod.rs`
- Create: `crates/kirin-prettyless/src/document/builder.rs`
- Create: `crates/kirin-prettyless/src/document/ir_render.rs`

**Step 1: Create files**

`builder.rs` — lines 1-129: imports, `Document<'a, L>` struct, `impl Document<'a, L>` (new, with_global_symbols, global_symbols, indent, block_indent, config, stage, list, render).

`ir_render.rs` — lines 130-345: `impl<'a, L: Dialect + PrettyPrint> Document<'a, L>` (print_statement, print_block, print_region, print_specialized_function, print_staged_function, print_function_header, print_stage_header, print_specialize_header, print_fn_signature, function_symbol_text, stage_symbol_text, resolve_global_symbol), `impl Deref for Document` (lines 326-333), `strip_trailing_whitespace()` (lines 335-345).

Tests (lines 347-376) go in a `tests.rs` file.

`mod.rs`:
```rust
mod builder;
mod ir_render;

pub use builder::Document;

#[cfg(test)]
mod tests;
```

**Step 2: Verify**

Run: `cargo build -p kirin-prettyless`
Run: `cargo nextest run -p kirin-prettyless`

**Step 3: Commit**

```
refactor(prettyless): split document.rs into document/ module
```

---

### Task 16: Final workspace verification

**Step 1: Full workspace build**

Run: `cargo build --workspace`

**Step 2: Full workspace test**

Run: `cargo nextest run --workspace`
Run: `cargo test --doc --workspace`

**Step 3: Format check**

Run: `cargo fmt --all`

**Step 4: Final commit (if fmt changes anything)**

```
chore: format after module reorganization
```
