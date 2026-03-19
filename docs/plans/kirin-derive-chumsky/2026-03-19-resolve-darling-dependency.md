# Resolve Direct darling Dependency in kirin-derive-chumsky

## Problem

`kirin-derive-chumsky/Cargo.toml` line 13 has `darling.workspace = true` as a direct dependency, violating the project convention documented in AGENTS.md: "Derive crates that depend on `kirin-derive-toolkit` must use `kirin_derive_toolkit::prelude::darling` -- never import `darling` directly."

The other two derive crates (`kirin-derive-ir` and `kirin-derive-interpreter`) correctly use only `kirin_derive_toolkit::prelude::darling` without a direct darling dependency.

## Research Findings

### Current darling versions in workspace

Only darling 0.23 exists in the dependency tree (verified via `cargo tree -i darling`). The AGENTS.md mentions darling 0.20 via `bon`, but bon-macros v3.9.1 now uses darling 0.23 too. So the two-version concern is currently moot, but the convention exists to prevent future version splits.

### How kirin-derive-chumsky uses darling

**Direct `use darling::*` imports** (from the direct dependency):
- `src/attrs.rs:3`: `use darling::{FromDeriveInput, FromField, FromVariant};`

**Re-exported `use kirin_derive_toolkit::prelude::darling` imports**:
- `src/lib.rs:19`: `use kirin_derive_toolkit::prelude::darling::{self, FromDeriveInput};`

**Usage of `darling::` path** (could come from either source):
- `src/input.rs:14,28,37,73,85`: `darling::Result`, `darling::Error::custom`, `darling::Error`
- `src/lib.rs:36,66`: `darling::Result`

### The proc-macro derive resolution question

The key issue is whether `#[derive(FromDeriveInput)]`, `#[derive(FromVariant)]`, and `#[derive(FromField)]` in `src/attrs.rs` resolve through the re-export path. These structs use:

```rust
#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(chumsky))]
pub struct ChumskyGlobalAttrs { ... }
```

The `#[derive(FromDeriveInput)]` macro is a proc-macro exported by the `darling` crate. When you write `use darling::FromDeriveInput;`, you import both the trait AND the derive macro. The question is whether `use kirin_derive_toolkit::prelude::darling::FromDeriveInput` also makes the derive macro available.

### How kirin-derive-ir and kirin-derive-interpreter handle this

**kirin-derive-ir** (`src/generate.rs`): Uses `kirin_derive_toolkit::prelude::darling` but does NOT derive `FromDeriveInput`/`FromVariant` on its own structs. It uses darling's API imperatively (calling `from_derive_input` etc.) rather than through derive macros.

**kirin-derive-interpreter** (`src/eval_call/layout.rs`): Uses `use kirin_derive_toolkit::prelude::darling::{self, FromDeriveInput, FromVariant}` AND derives `#[derive(FromDeriveInput)]` and `#[derive(FromVariant)]` on structs. This file does NOT have a direct darling dependency in its crate's Cargo.toml, so the derive macros must be resolving through the re-export.

This confirms that `kirin_derive_toolkit::prelude::darling::{FromDeriveInput, FromVariant, FromField}` correctly resolves both the traits and the derive macros. Rust's proc-macro re-export mechanism supports this.

### Verification from kirin-derive-interpreter

`kirin-derive-interpreter/Cargo.toml` has no direct darling dependency. `src/eval_call/layout.rs` uses:
```rust
use kirin_derive_toolkit::prelude::darling::{self, FromDeriveInput, FromVariant};

#[derive(FromDeriveInput)]
#[darling(attributes(interpret), allow_unknown_fields)]
pub(crate) struct EvalCallGlobalAttrs { ... }

#[derive(FromVariant)]
#[darling(attributes(interpret))]
pub(crate) struct EvalCallStatementAttrs { ... }
```

This proves the pattern works without a direct darling dependency.

## Proposed Changes

### 1. Remove direct darling dependency

**File**: `crates/kirin-derive-chumsky/Cargo.toml`

Remove line 13:
```diff
-darling.workspace = true
```

### 2. Update attrs.rs import

**File**: `crates/kirin-derive-chumsky/src/attrs.rs:3`

Change:
```diff
-use darling::{FromDeriveInput, FromField, FromVariant};
+use kirin_derive_toolkit::prelude::darling::{FromDeriveInput, FromField, FromVariant};
```

### 3. Verify remaining `darling::` paths resolve

All other `darling::` usages in `src/input.rs` and `src/lib.rs` already reach darling through `use kirin_derive_toolkit::prelude::darling::{self, ...}` which brings `darling` into scope as a module alias. These paths (`darling::Result`, `darling::Error`) will continue to work unchanged.

## Migration Steps

1. Change the import in `src/attrs.rs` from `use darling::` to `use kirin_derive_toolkit::prelude::darling::`.
2. Remove `darling.workspace = true` from Cargo.toml.
3. Run `cargo build -p kirin-derive-chumsky` to verify compilation.
4. Run `cargo nextest run -p kirin-derive-chumsky` to verify tests pass.
5. Run `cargo build --workspace` to verify no downstream breakage.

## Risk Assessment

- **Low risk**: kirin-derive-interpreter already proves this pattern works with `#[derive(FromDeriveInput)]` and `#[derive(FromVariant)]` resolved through the re-export path.
- **`#[darling(...)]` helper attribute**: The `#[darling(attributes(chumsky))]` helper attribute is resolved by the proc-macro crate at compile time, not through import paths. It will work regardless of how the derive macro is imported.
- **Workspace darling version**: Currently only 0.23. If a future dependency brings in a different version, the re-export through kirin-derive-toolkit ensures all derive crates use the same one.

## Testing Strategy

- `cargo build -p kirin-derive-chumsky` -- crate compiles
- `cargo nextest run -p kirin-derive-chumsky` -- snapshot tests and codegen tests pass
- `cargo build --workspace` -- no downstream crate breaks
- `cargo tree -p kirin-derive-chumsky | grep darling` -- darling appears only transitively through kirin-derive-toolkit, not as a direct dependency
