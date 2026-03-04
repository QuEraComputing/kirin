# Derive Framework Review Plan — 2026-03-04

**Scope:** `kirin-derive-core` (6,732 lines), `kirin-derive` (314 lines), `kirin-derive-dialect` (2,851 lines)
**Focus:** Downstream extensibility — can a third-party author create `#[derive(IsQuantum)]` that propagates through `#[wraps]` and composes with existing derives?

## Scope

### Crate Architecture

```
kirin-derive-core (library, 6,732 lines)
├── ir/         — Input<L>, Layout trait, #[kirin(...)] attrs, #[wraps] parsing
├── scan.rs     — Scan<'ir, L> visitor trait
├── emit.rs     — Emit<'ir, L> code generation trait
├── generators/ — builder, field iter, property, marker, stage_info
├── tokens/     — Code token builders (TraitMethodImplTokens, WrapperCallTokens, etc.)
└── codegen/    — Constructor, generics builder, field bindings

kirin-derive (proc-macro, 314 lines)
├── #[derive(Dialect)] — calls all generators (field iters + properties + builder + marker)
├── 10 field iter derives — HasArguments/Mut, HasResults/Mut, etc.
├── 4 property derives — IsTerminator, IsConstant, IsPure, IsSpeculatable
└── #[derive(StageMeta)]

kirin-derive-dialect (library, 2,851 lines)
├── Mirrors kirin-derive-core/generators/ exactly
└── Purpose: re-export generators for downstream derive macro authors
```

### Key Extension Points

1. **`Layout` trait** (`ir/layout.rs`) — 4 associated types for extending IR metadata
2. **`DeriveProperty`** (`generators/property/context.rs`) — Configurable property generator
3. **`PropertyKind` enum** — Currently hardcoded to 4 variants (Constant, Pure, Speculatable, Terminator)
4. **`WrapperCallTokens`** (`tokens/wrapper.rs`) — Generic delegation code generation
5. **`Scan<'ir, L>` + `Emit<'ir, L>`** — Visitor/emitter traits parameterized by Layout

### The "IsQuantum" Test Case

A downstream author wants to:
1. Define `trait IsQuantum { fn is_quantum(&self) -> bool; }`
2. Create `#[derive(IsQuantum)]` that generates the impl
3. Support `#[kirin(quantum)]` attribute on variants
4. Have `#[wraps]` variants automatically delegate to `<Inner as IsQuantum>::is_quantum(inner)`

**Current path:** Use `DeriveProperty::new(kind, crate, trait, method, type)` from `kirin-derive-dialect`. But `PropertyKind` is a closed enum — there's no variant for `Quantum`. The property generator reads `statement.attrs.constant/pure/speculatable/terminator` via `PropertyKind::statement_value()`, which requires a match arm.

## Reviewer Roster

| Reviewer | Themes | Primary Files | Secondary Files |
|----------|--------|---------------|-----------------|
| PL Theorist | Abstractions & Type Design | `ir/layout.rs`, `scan.rs`, `emit.rs`, `generators/property/` | `ir/attrs.rs`, `ir/statement/definition.rs` |
| Compiler Engineer | Performance & Scalability | `kirin-derive-dialect/` (full), `generators/` (full) | `Cargo.toml` files, crate dependencies |
| Rust Engineer | Correctness & Safety, Code Quality | `generators/property/context.rs`, `generators/property/scan.rs`, `generators/property/emit.rs`, `tokens/wrapper.rs` | `generators/field/`, `generators/builder/` |
| Physicist | API Ergonomics & Naming | `kirin-derive/src/lib.rs`, `generators/property/context.rs` | `ir/layout.rs`, `kirin-derive-interpreter/src/` (as downstream example) |

## Themes

All five themes apply, with emphasis on Abstractions and Ergonomics:

1. **Abstractions & Type Design** — Is `Layout` sufficient for downstream extension? Is `PropertyKind` appropriately open/closed? Can `Scan`/`Emit` compose for custom properties?
2. **API Ergonomics & Naming** — What's the minimal path for a downstream author to create `#[derive(IsQuantum)]`? How many concepts must they learn?
3. **Performance & Scalability** — Is the kirin-derive-core ↔ kirin-derive-dialect duplication necessary? What's the compilation cost?
4. **Correctness & Safety** — Does wrapper delegation correctly propagate arbitrary traits? Are there edge cases?
5. **Code Quality & Idioms** — Is the property generator API idiomatic? Could it be more ergonomic?

## Design Context (for reviewer prompts)

### Derive Infrastructure Conventions (from AGENTS.md)
- **Darling re-export rule**: Derive crates must use `kirin_derive_core::prelude::darling` — never import darling directly.
- **Helper attribute pattern**: `#[wraps]` and `#[callable]` are intentionally separate from `#[kirin(...)]` for composability.
- **Custom Layout for derive-specific attributes**: When a derive macro needs attributes beyond `StandardLayout`, define a custom `Layout` impl. See `EvalCallLayout` in `kirin-derive-interpreter`.
- **`#[kirin(...)]` attribute convention**: Use path syntax for `crate`: `#[kirin(crate = kirin_ir)]`.

### Won't Fix from Prior Reviews
- None for this scope (first review of derive infrastructure)

### Key Architecture Decision
`kirin-derive-dialect` duplicates `kirin-derive-core/generators/` intentionally — it's a library for downstream derive macro authors, avoiding proc-macro crate dependency issues.
