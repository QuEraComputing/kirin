# Feature-Gate kirin-prettyless in kirin-chumsky

## Problem

`kirin-chumsky` unconditionally depends on `kirin-prettyless` (Cargo.toml line 10: `kirin-prettyless.workspace = true`). This means any crate that needs only parsing capabilities also pulls in the entire pretty-printing stack. The `PrettyPrint` trait, its re-exports, and one impl on `SymbolName` are the only actual uses. These should be behind an optional `pretty` feature.

## Research Findings

### Direct prettyless usage in kirin-chumsky/src

1. **`lib.rs:66`**: `pub use kirin_prettyless::PrettyPrint;` -- top-level re-export of the trait
2. **`lib.rs:74`**: `pub use kirin_derive_chumsky::PrettyPrint;` -- derive macro re-export (already behind `#[cfg(feature = "derive")]`)
3. **`lib.rs:90`**: `pub use kirin_prettyless::prelude::*;` -- inside `pub mod prelude`
4. **`lib.rs:93`**: `pub use kirin_derive_chumsky::{HasParser, PrettyPrint};` -- derive re-export in prelude (already behind derive feature)
5. **`ast/symbols.rs:3`**: `use kirin_prettyless::{ArenaDoc, DocAllocator, Document, PrettyPrint};` -- used for `impl PrettyPrint for SymbolName`
6. **`function_text/tests.rs:8`**: `use kirin_prettyless::PrintExt;` -- test-only usage

### What `kirin_prettyless::prelude::*` re-exports

`ArenaDoc`, `Config`, `DocAllocator`, `Document`, `FunctionRenderBuilder`, `PipelineDocument`, `PipelinePrintExt`, `PipelineRenderBuilder`, `PrettyPrint`, `PrettyPrintExt`, `PrintExt`, `RenderDispatch`, `RenderError`, plus `prettyless` (the underlying crate).

### Downstream consumers of these re-exports

- Dialect crates use `kirin::prelude::*` which includes `kirin_chumsky::prelude::*`, pulling in all prettyless types. After the dialect decoupling (sister plan), dialects would import prettyless directly.
- `kirin-test-utils` depends on `kirin-chumsky` and separately on `kirin-prettyless`.
- `kirin-test-languages` depends on `kirin-chumsky` with derive feature.
- The top-level `kirin` crate depends on both `kirin-chumsky` and `kirin-prettyless` separately.

### The `SymbolName` PrettyPrint impl

`ast/symbols.rs` contains `impl PrettyPrint for SymbolName<'src>` which uses `Document`, `ArenaDoc`, `DocAllocator` from kirin-prettyless. This is the only non-re-export runtime code in kirin-chumsky that depends on prettyless. It renders `@name` symbols during pretty printing.

### The `PrettyPrint` derive macro hosting

kirin-chumsky re-exports `kirin_derive_chumsky::PrettyPrint` (the derive macro). The derive macro itself lives in kirin-derive-chumsky and generates code that references `::kirin::parsers` (the kirin-chumsky crate path) for some types and `kirin_prettyless` for others. The derive macro is already behind `#[cfg(feature = "derive")]`.

## Proposed Changes

### Cargo.toml

```toml
[dependencies]
kirin-prettyless = { workspace = true, optional = true }

[features]
default = ["pretty"]
pretty = ["kirin-prettyless"]
derive = ["kirin-derive-chumsky"]
```

### lib.rs changes

```rust
// Line 66: conditional re-export
#[cfg(feature = "pretty")]
pub use kirin_prettyless::PrettyPrint;

// Line 74: already behind derive feature, add pretty gate too
#[cfg(all(feature = "derive", feature = "pretty"))]
pub use kirin_derive_chumsky::PrettyPrint;

// In prelude module:
#[cfg(feature = "pretty")]
pub use kirin_prettyless::prelude::*;

#[cfg(all(feature = "derive", feature = "pretty"))]
pub use kirin_derive_chumsky::PrettyPrint;
// Keep non-pretty derive re-export:
#[cfg(feature = "derive")]
pub use kirin_derive_chumsky::HasParser;
```

### ast/symbols.rs changes

Gate the `PrettyPrint` impl:

```rust
#[cfg(feature = "pretty")]
use kirin_prettyless::{ArenaDoc, DocAllocator, Document, PrettyPrint};

#[cfg(feature = "pretty")]
impl<'src> PrettyPrint for SymbolName<'src> {
    // ... existing impl unchanged
}
```

### function_text/tests.rs changes

The `use kirin_prettyless::PrintExt` line is in test code. Options:
- Add `kirin-prettyless` as a dev-dependency (not optional in dev-deps)
- Gate the test with `#[cfg(feature = "pretty")]`

Recommendation: keep `kirin-prettyless` as a dev-dependency for tests, since roundtrip tests need printing.

### Impact on the `PrettyPrint` derive macro

The `PrettyPrint` derive (in kirin-derive-chumsky) generates code referencing `kirin_prettyless` types. When a user applies `#[derive(PrettyPrint)]`, they need `kirin-prettyless` available. This is orthogonal to this change -- the derive macro is a proc-macro crate and doesn't depend on kirin-prettyless at build time. The generated code references it, so the downstream crate must have it in scope. This works naturally: if a dialect enables `pretty` feature on kirin-chumsky, kirin-prettyless is available.

## Migration Steps

1. Make `kirin-prettyless` optional in kirin-chumsky's Cargo.toml with `pretty` feature.
2. Add `pretty` to `default` features for backward compatibility.
3. Gate all `kirin_prettyless` imports and re-exports behind `#[cfg(feature = "pretty")]`.
4. Gate `SymbolName`'s `PrettyPrint` impl behind `#[cfg(feature = "pretty")]`.
5. Add `kirin-prettyless` as a dev-dependency (non-optional) for test code.
6. Verify `cargo build -p kirin-chumsky --no-default-features` compiles (parser-only).
7. Verify `cargo build -p kirin-chumsky` compiles (parser + pretty, default).
8. Update the top-level `kirin` crate's dependency on kirin-chumsky to not need any extra features (it already depends on kirin-prettyless separately).
9. Coordinate with dialect decoupling plan: once dialects depend on kirin-chumsky directly, they can choose `parser`-only or `parser`+`pretty`.

## Risk Assessment

- **Name collision**: When `pretty` feature is off, `PrettyPrint` (trait) is unavailable from kirin-chumsky. Any downstream code doing `use kirin_chumsky::PrettyPrint` breaks. Mitigated by `default = ["pretty"]`.
- **Derive macro without trait**: If someone enables `derive` but not `pretty`, `#[derive(PrettyPrint)]` would generate code that imports `kirin_prettyless::PrettyPrint` but the trait isn't re-exported from kirin-chumsky. This is fine -- the generated code uses absolute paths (`::kirin::parsers::PrettyPrint` or `kirin_prettyless::PrettyPrint`), not relative imports through kirin-chumsky. However, the user must have kirin-prettyless as a dependency. Consider: should `derive` automatically enable `pretty`? Probably not -- `derive` is for `HasParser` derive too. A `derive-pretty` compound feature may be warranted.
- **Conditional prelude**: `kirin_chumsky::prelude::*` without `pretty` would not include printing types. Code using `use kirin_chumsky::prelude::*` and then calling `PrettyPrint` methods would fail. This is expected behavior.

## Testing Strategy

- `cargo build -p kirin-chumsky --no-default-features` -- parser-only build, no prettyless
- `cargo build -p kirin-chumsky --features derive` -- derive without pretty
- `cargo build -p kirin-chumsky` -- default (pretty enabled)
- `cargo nextest run -p kirin-chumsky` -- all tests pass with default features
- `cargo build --workspace` -- no workspace-wide breakage
- Check that kirin-test-languages and kirin-test-utils still compile
