# Derive Framework Review — 2026-03-04

**Scope:** `kirin-derive-core` (6,732 lines), `kirin-derive` (314 lines), `kirin-derive-dialect` (2,851 lines)
**Reviewers:** PL Theorist, Compiler Engineer, Rust Engineer, Physicist
**Plan:** docs/plans/2026-03-04-derive-framework-review-plan.md
**Focus:** Downstream extensibility for custom property traits (e.g., `IsQuantum`)

## Correctness & Safety

[P1] [confirmed] **Accepted** `kirin-derive-dialect/src/property/scan.rs` diverges from `kirin-derive-core/src/generators/property/scan.rs` — the dialect copy only calls `validate_speculatable_pure_invariant` but omits `validate_constant_pure_invariant`. The two copies enforce different semantic invariants. Since `kirin-derive-dialect` is not in the workspace, this is currently unreachable — but if the crate is revived, the missing validation would silently allow `#[kirin(constant)]` without `#[kirin(pure)]`. — `kirin-derive-dialect/src/property/scan.rs:13` [PL Theorist]

## Abstractions & Type Design

[P1] [confirmed] **Accepted** `PropertyKind` is a closed enum with 4 hardcoded variants (`Constant`, `Pure`, `Speculatable`, `Terminator`). `DeriveProperty` reads attribute values through `PropertyKind::statement_value()` which matches on these variants. A downstream author wanting `#[derive(IsQuantum)]` cannot add a `Quantum` variant without modifying `kirin-derive-core`. The `DeriveProperty` constructor API *looks* open (takes `PropertyKind`, trait path, method name) but is closed in practice. — `generators/property/context.rs:9-34` [PL Theorist, Rust Engineer, Physicist]
*User note: also think about more complicated use cases — e.g., kirin-chumsky-derive implements `HasParser` derive with a text format string, not just a bool property. The extensibility mechanism should support complex derives too.*
*User note: `PropertyKind::Constant` was borrowed from MLIR because MLIR doesn't have a generic constant statement — it uses a trait/interface. In Kirin, `kirin-constant` is the canonical place for constants. Could remove this property or make PropertyKind more generic so downstream developers can add their own. Consider using a trait instead of closures for the open dispatch mechanism.*

[P1] [confirmed] **Accepted** `KirinStructOptions`, `KirinEnumOptions`, and `StatementOptions` in `attrs.rs` are closed structs with hardcoded bool fields (`constant`, `pure`, `speculatable`, `terminator`). The `Layout` extension point (`ExtraGlobalAttrs`, `ExtraStatementAttrs`) exists but `DeriveProperty` bypasses it — it reads directly from `StandardLayout`-typed `statement.attrs.*` fields. The two extension mechanisms are not unified: `Layout` works for `EvalCallLayout`-style derives but not for property-style derives. — `ir/attrs.rs:85-99`, `generators/property/scan.rs:6` [Physicist, PL Theorist]

[P1] [confirmed] **Accepted** `kirin-derive-dialect` is not listed in `workspace.members` in `Cargo.toml` and is depended on by zero workspace crates. It contains ~2,851 lines of duplicated generators from `kirin-derive-core/src/generators/`. The stated intent (core = toolkit, dialect = Dialect generators) is not realized — `kirin-derive-core` carries both the toolkit and the dialect-specific generators, while `kirin-derive-dialect` is dead code. Either complete the migration (move generators to `kirin-derive-dialect`, make core purely generic) or delete the dead crate. — `Cargo.toml`, `kirin-derive-dialect/` [Compiler Engineer, PL Theorist]

[P2] [confirmed] **Accepted** `Layout` extensibility is nominal, not compositional. A downstream author wanting only `ExtraFieldAttrs` must define all 4 associated types. No partial-override pattern or "extend StandardLayout on one axis" mechanism is provided. — `ir/layout.rs:3-18` [PL Theorist]

[P2] [confirmed] **Accepted** `#[wraps]` field-category recognition relies on type-name string matching (`Collection::from_type(ty, "SSAValue")`). A downstream dialect that newtype-wraps `SSAValue` under a different name will silently fall through to `FieldData::Value`. The structural `#[wraps]` and type-name-based field-category mechanisms are orthogonal but their interaction is undocumented. — `ir/statement/definition.rs:119-182` [PL Theorist]
*User note: no fix found in the past. Potential solutions to explore:*
1. *Explicit field-category attribute: `#[kirin(category = argument)]` overrides type-name detection*
2. *Marker trait approach: `impl IsIRArgument for MySSA {}` checked at compile time*
3. *Type alias registry: register type aliases in `#[kirin(...)]` so `MySSA` maps to `SSAValue` category*

## Performance & Scalability

[P2] [likely] **Accepted** Each `DeriveProperty` invocation independently re-parses and re-scans the input. With `#[derive(Dialect)]` applying 4 property derives + 10 field iter derives + builder + marker = 16 scan/emit passes per type. With 10+ custom property derives on a large enum, parsing cost multiplies. No shared parse cache across derive invocations (proc-macro isolation). — `generators/property/context.rs:60-68` [Compiler Engineer]
*User note: not very severe. Nice to fix but not worth introducing a lot of lifetime logic.*

[P3] [likely] **Accepted** `stage_info.rs` deduplicates dialects via `Vec::contains` (O(n^2)) when the `BTreeMap` already built could serve as the deduplication structure. Negligible at current scale. — `generators/stage_info.rs:59-68` [Compiler Engineer]

## API Ergonomics & Naming

[P2] [confirmed] **Accepted** A downstream author trying to create `#[derive(IsQuantum)]` driven by `#[kirin(quantum)]` has no path without upstream changes to at least: `PropertyKind` (enum + 2 match arms), `KirinStructOptions`/`KirinEnumOptions`/`StatementOptions` (3 structs), and 2 `From` impls — roughly 5 sites across 2 files. The cleaner path is to bypass `DeriveProperty` entirely and write a standalone proc-macro using `Layout`+`EvalCallLayout` as a pattern reference. — `kirin-derive/src/lib.rs`, `generators/property/context.rs` [Physicist]
*User note: should think about how to reduce this overhead and reuse kirin-derive-core as much as possible.*

[P3] [confirmed] **Accepted** The crate naming creates a false sense that `kirin-derive-core` is a stable extensible toolkit when it is tightly coupled to the 4 built-in properties. A downstream author expecting extension-ready infrastructure in "core" finds dialect-specific vocabulary (`PropertyKind::Constant`) baked in. — `generators/property/context.rs:1` [Physicist]
*User note: PropertyKind::Constant was borrowed from MLIR. In Kirin, kirin-constant is the canonical constant dialect. Could remove this property or make PropertyKind generic for downstream extension.*

## Code Quality & Idioms

[P2] [confirmed] **Accepted** `WrapperCallTokens` hardcodes `&self` receiver semantics — the generated UFCS call is always `<WrapperTy as Trait>::method(field)`. No way to express `&mut self` delegation or extra arguments. Should document this assumption or add optional `extra_args`. — `tokens/wrapper.rs:17` [Rust Engineer]

[P3] [confirmed] **Accepted** `PropertyKind` could be eliminated by replacing `global_value`/`statement_value` with two closures `Fn(&Input<L>) -> bool` / `Fn(&Statement<L>) -> bool` stored in `DeriveProperty`. This would make `DeriveProperty` truly open without closed dispatch. — `generators/property/context.rs:9-34` [Rust Engineer]
*User note: consider using a trait instead of closures — may be more natural for users implementing custom properties.*

[P3] [likely] **Accepted** Builder `build()` methods call `.expect("... is required")` unconditionally. Panics surface as proc-macro ICEs with no span information. Internal-only, low risk in practice, but violates the graceful-error principle for proc-macros. — `tokens/wrapper.rs:62-65`, `tokens/trait_impl.rs:119-131` [Rust Engineer]

[P3] [uncertain] **Accepted** `Scan` and `Emit` visitor protocols cannot be composed — no monoidal composition of visitors. Standard limitation of visitor patterns, acceptable at current scale. — `scan.rs`, `emit.rs` [PL Theorist]
*User note: open to aggressive refactor on this abstraction level. Possible alternatives:*
1. *Fold-based architecture: replace visitor with a `Fold<Acc>` trait that threads an accumulator, composable via `(FoldA, FoldB)` tuples*
2. *Query-based architecture: each generator registers queries ("give me all fields of category Argument"), a central driver resolves them in one pass, results cached in a shared context*
3. *Salsa-style incremental: use `salsa` or similar to memoize IR parsing, derive generators become queries that read from the database*
4. *Tagless-final / algebra-based: each field category is an algebra method, compose algebras via product types. More principled but higher learning curve.*

## Cross-Cutting Themes

1. **`DeriveProperty` is closed, not extensible** — identified by 4 reviewers across Abstractions, Ergonomics, Code Quality. The `PropertyKind` enum, `StandardLayout` attrs, and `DeriveProperty` constructor all conspire to make custom property derives impossible without upstream changes. This is the dominant finding. User wants to support both simple bool properties (IsQuantum) and complex derives (HasParser with format strings).

2. **`kirin-derive-dialect` is dead code** — identified by 2 reviewers (PL Theorist, Compiler Engineer). The intended toolkit/dialect split is not realized. Generators live in `kirin-derive-core`, the dialect crate is not in the workspace.

3. **`Layout` extension point is underutilized** — identified by 2 reviewers (PL Theorist, Physicist). The `Layout` trait exists for extending IR metadata but `DeriveProperty` bypasses it, reading `StandardLayout` directly. The two extension mechanisms are not unified.

## Summary

- P0: 0 issues
- P1: 3 issues — all accepted
- P2: 5 improvements — all accepted
- P3: 5 notes — all accepted

Confirmed: 10 | Likely: 2 | Uncertain: 1

## Filtered Findings

<details>
<summary>1 finding filtered</summary>

- [P4] `StatementBuilder` is a zero-field unit struct with a `new()` constructor — filtered because P4 is below threshold and the finding is stylistic only.
</details>
