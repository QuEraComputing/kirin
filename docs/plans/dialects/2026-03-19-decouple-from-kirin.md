# Decouple Dialect Crates from Top-Level `kirin`

## Problem

All 7 dialect crates (`kirin-arith`, `kirin-bitwise`, `kirin-cf`, `kirin-cmp`, `kirin-constant`, `kirin-function`, `kirin-scf`) depend on `kirin.workspace = true`, which pulls in the full `kirin` crate including `kirin-chumsky`, `kirin-prettyless`, `kirin-lexer`, and `chumsky`. This means any crate that only needs IR types and derive macros still transitively compiles the entire parser and printer stack.

Dialects should depend on `kirin-ir` directly for IR types, with parser (`HasParser`/`PrettyPrint`) support feature-gated behind optional dependencies on `kirin-chumsky` and `kirin-prettyless`.

## Research Findings

### What `kirin` re-exports (src/lib.rs)

```
pub use kirin_chumsky as parsers;
pub use kirin_ir as ir;
pub use kirin_prettyless as pretty;
pub mod prelude { kirin_chumsky::prelude::* + kirin_ir::* }
```

### Dialect import patterns

All 7 dialects use `use kirin::prelude::*` for their main source, which brings in both `kirin_ir::*` and `kirin_chumsky::prelude::*`. From `kirin_ir::*` they use: `Dialect`, `SSAValue`, `ResultValue`, `Successor`, `Region`, `Block`, `Symbol`, `CompileTimeValue`, `HasStageInfo`, `Typeof`, `Placeholder`, plus IR property traits. From `kirin_chumsky::prelude::*` they use: `HasParser`, `HasDialectParser`, `PrettyPrint` (trait), `EmitIR`, `DirectlyParsable`, `BoxedParser`, `TokenInput`, `Token`, `chumsky::prelude::*`, plus the `PrettyPrint` and `HasParser` derive macros.

### Per-dialect breakdown

**Simple dialects** (kirin-bitwise, kirin-cf, kirin-cmp, kirin-scf): Only use `kirin::prelude::*` in lib.rs. The `#[derive(Dialect, HasParser, PrettyPrint)]` on the main enum/struct drives all parser/printer needs. Their interpret_impl.rs files use only `kirin::prelude::{CompileTimeValue, HasStageInfo}` (pure IR traits).

**kirin-constant**: Same as simple, plus uses `kirin::pretty::{ArenaDoc, DocAllocator, Document, PrettyPrint}` in tests.

**kirin-arith**: The main enum is simple, but `ArithType` and `ArithValue` in `src/types/` have hand-written `HasParser` and `PrettyPrint` impls. These files use:
- `kirin::ir::{Dialect, Typeof, Placeholder}`
- `kirin::parsers::chumsky::prelude::*`
- `kirin::parsers::{BoxedParser, DirectlyParsable, HasParser, PrettyPrint, Token, TokenInput}`
- `kirin::pretty::{ArenaDoc, DocAllocator, Document}`

**kirin-function**: Multiple sub-modules (body.rs, call.rs, lambda.rs, bind.rs, ret.rs) all use `kirin::prelude::*`. Inline tests use `kirin::ir::{HasArguments, HasBlocks, ...}`.

### Derive macro crate path defaults

`kirin-derive-chumsky` defaults to `::kirin::parsers` for generated code paths. The `#[chumsky(crate = kirin_chumsky)]` attribute overrides this. Similarly, `kirin-derive-ir` defaults to `::kirin::ir` with `#[kirin(crate = kirin_ir)]` override.

### Current dependency chain
```
dialect -> kirin -> kirin-chumsky -> kirin-prettyless -> kirin-ir
                 -> kirin-prettyless
                 -> kirin-ir
                 -> kirin-lexer
                 -> chumsky
```

### Target dependency chain
```
dialect -> kirin-ir                              (always)
        -> kirin-chumsky   (optional, "parser")  -> kirin-prettyless -> kirin-ir
        -> kirin-prettyless (optional, "pretty")  -> kirin-ir
        -> kirin-interpreter (optional, "interpret") -- already done
```

## Proposed Changes

### Feature flag design

Each dialect gets three optional features: `parser`, `pretty`, `interpret` (interpret already exists). A `default` feature enables `parser` + `pretty` for backward compatibility.

```toml
[features]
default = ["parser", "pretty"]
parser = ["kirin-chumsky"]
pretty = ["kirin-prettyless"]
interpret = ["kirin-interpreter", ...]
```

### Source code changes

1. **Split `use kirin::prelude::*`** into explicit imports:
   - `use kirin_ir::*;` (always available)
   - `#[cfg(feature = "parser")] use kirin_chumsky::prelude::*;` (parser types)
   - The derive macros (`Dialect`, `HasParser`, `PrettyPrint`) come from `kirin_ir` and `kirin_chumsky` respectively.

2. **Conditionally derive `HasParser` and `PrettyPrint`**:
   - The main dialect enum/struct derive lines change from:
     ```rust
     #[derive(Dialect, HasParser, PrettyPrint)]
     ```
     to:
     ```rust
     #[derive(Dialect)]
     #[cfg_attr(feature = "parser", derive(HasParser))]
     #[cfg_attr(feature = "pretty", derive(PrettyPrint))]
     ```
   - `#[chumsky(...)]` attributes need `#[cfg_attr(feature = "parser", chumsky(...))]` guards.

3. **Add crate path overrides** on all derive attributes since the default `::kirin::parsers` path no longer exists:
   - `#[chumsky(crate = kirin_chumsky)]` on each type
   - `#[kirin(crate = kirin_ir)]` on each type
   - `#[pretty(crate = kirin_prettyless)]` on each type

4. **kirin-arith types** (`ArithType`, `ArithValue`): Their hand-written `HasParser` and `PrettyPrint` impls need `#[cfg(feature = "parser")]` and `#[cfg(feature = "pretty")]` guards respectively. The imports from `kirin::parsers::*` and `kirin::pretty::*` get the same guards.

5. **Interpret impls**: Already gated behind `#[cfg(feature = "interpret")]`. Change `use kirin::prelude::{CompileTimeValue, HasStageInfo}` to `use kirin_ir::{CompileTimeValue, HasStageInfo}`.

6. **Test modules**: Dialect inline tests (e.g., `kirin-cf/src/tests.rs`, `kirin-function/src/call.rs`) that use `kirin::ir::*` change to `kirin_ir::*`.

### Cargo.toml changes per dialect

**kirin-arith** (representative, others similar):
```toml
[dependencies]
kirin-ir = { workspace = true }
kirin-chumsky = { workspace = true, optional = true }
kirin-prettyless = { workspace = true, optional = true }
kirin-interpreter = { workspace = true, optional = true }

[features]
default = ["parser", "pretty"]
parser = ["kirin-chumsky"]
pretty = ["kirin-prettyless"]
interpret = ["kirin-interpreter"]
```

**kirin-cf, kirin-scf** (has smallvec for interpret):
```toml
[dependencies]
kirin-ir = { workspace = true }
kirin-chumsky = { workspace = true, optional = true }
kirin-prettyless = { workspace = true, optional = true }
kirin-interpreter = { workspace = true, optional = true }
smallvec = { workspace = true, optional = true }

[features]
default = ["parser", "pretty"]
parser = ["kirin-chumsky"]
pretty = ["kirin-prettyless"]
interpret = ["kirin-interpreter", "smallvec"]
```

### Downstream impact

- **kirin-test-languages**: Depends on dialect crates. Its existing feature flags (`arith-function-language`, etc.) should activate `parser` + `pretty` features on the dialects they compose.
- **Top-level `kirin` crate**: Its `[dev-dependencies]` on dialect crates should use default features (unchanged).
- **Roundtrip tests** (in workspace `tests/`): Use `kirin-test-languages` which enables parser+pretty. No change needed.
- **toy-lang example**: Depends on dialect crates through `kirin`. Will need to enable `parser` + `pretty` features explicitly or through `kirin` re-exports.

## Migration Steps

1. **Start with kirin-constant** (simplest struct dialect, no custom parser/printer impls). Validate the pattern compiles and tests pass.
2. **kirin-cmp** (simple enum, same pattern as constant).
3. **kirin-bitwise** (simple enum, same pattern).
4. **kirin-cf** (enum with smallvec for interpret).
5. **kirin-scf** (enum + structs with Block fields, smallvec for interpret).
6. **kirin-function** (multiple sub-modules, most complex source changes).
7. **kirin-arith** (has hand-written HasParser/PrettyPrint impls, most conditional compilation needed).
8. **Update kirin-test-languages** feature flags to propagate parser/pretty to dialects.
9. **Update workspace Cargo.toml** dev-dependency feature flags if needed.

### Interaction with feature-gating prettyless in kirin-chumsky (P1-2)

If kirin-chumsky gets a `pretty` feature that gates its kirin-prettyless dependency (see sister plan), then the dialect `parser` feature would NOT transitively pull in prettyless. This is the ideal layering:
- `parser` feature -> kirin-chumsky (no prettyless) -> kirin-ir
- `pretty` feature -> kirin-prettyless -> kirin-ir
- `parser` + `pretty` -> full stack

However, kirin-chumsky currently re-exports `PrettyPrint` unconditionally and uses it in `ast/symbols.rs`. Until P1-2 lands, the `parser` feature will still transitively bring in prettyless. Migration order: do this plan first (decouple from `kirin`), then P1-2 (feature-gate prettyless in chumsky) to achieve full separation.

## Risk Assessment

- **cfg_attr on derive attributes**: `#[cfg_attr(feature = "parser", chumsky(format = "..."))]` is unusual. If `chumsky` attribute is not recognized when the feature is off, it may warn or error. Need to verify that unused attributes are silently ignored (they should be, since the derive macro that consumes them is also absent).
- **Derive macro crate path**: Every dialect type needs `#[chumsky(crate = kirin_chumsky)]` and `#[kirin(crate = kirin_ir)]`. Missing one causes a compile error referencing `::kirin::parsers` or `::kirin::ir` which won't exist.
- **Feature combinations**: Must test that each dialect compiles with: no features, parser-only, pretty-only, interpret-only, and all features.
- **Downstream breakage**: Any external code doing `use kirin_arith::Arith` and expecting `HasParser` to be implemented will break without `parser` feature. The `default` feature mitigates this.
- **kirin-constant's `PrettyPrint` bound**: `Constant<T: ... + PrettyPrint, Ty>` has `PrettyPrint` as a trait bound on the struct itself. This bound comes from `kirin_prettyless::PrettyPrint`. When `pretty` feature is off, this type definition would fail. Options: (a) always depend on kirin-prettyless for the trait only, (b) feature-gate the entire struct behind `pretty`, (c) define a local `PrettyPrint` stub. This needs careful design.

## Testing Strategy

- `cargo build -p kirin-arith --no-default-features` -- IR-only build
- `cargo build -p kirin-arith --features parser` -- parser but no pretty
- `cargo build -p kirin-arith --features pretty` -- pretty but no parser
- `cargo build -p kirin-arith` -- default (parser + pretty)
- `cargo build -p kirin-arith --all-features` -- everything including interpret
- Repeat for all 7 dialects
- `cargo nextest run --workspace` -- full workspace tests still pass
- `cargo test --doc --workspace` -- doc tests still pass
