# U2: Parser Runtime -- Formalism Review

## Findings

### [P1] [confirmed] `EmitContext` conflates two scoping disciplines in a single mutable map -- traits/emit_ir.rs:36

`EmitContext` uses flat `FxHashMap<String, SSAValue>` and `FxHashMap<String, Block>` maps without lexical scoping. When a `Region` emits multiple blocks, inner blocks can shadow outer SSA names silently -- there is no scope-push/scope-pop mechanism. For nested constructs (a block inside a region inside a function body), a name collision between an outer block argument and an inner block argument will cause the outer binding to be permanently overwritten. This violates the standard SSA dominance property that each name has exactly one definition visible at any given scope level.

In standard compiler infrastructure (MLIR, Cranelift), SSA name resolution uses a *scope stack* or *nested symbol table*. The current flat map makes scoping correctness dependent on external callers (e.g., `Region::emit_with` must carefully not re-use names), which is fragile.

**Alternative formalisms:**

| Approach | Shadowing safe | Perf | Complexity |
|----------|---------------|------|------------|
| Scope stack (`Vec<HashMap>` with push/pop) | Yes | O(depth) lookup | Low |
| De Bruijn indices (no names, positional) | Yes (by construction) | O(1) | High (breaks text format) |
| Current flat map | No | O(1) | Low |

**Suggested action:** Add `push_scope()` / `pop_scope()` methods to `EmitContext` that push/pop a new map layer. Inner scopes shadow outer names but outer names are restored on pop. This is the standard approach in MLIR's `OpAsmParser`.

**References:** MLIR OpAsmParser SSA scope stack; Appel, "Modern Compiler Implementation," Ch. 5 (symbol tables).

### [P2] [likely] `HasDialectEmitIR` witness trait is a workaround for missing trait-level GAT normalization -- traits/has_dialect_emit_ir.rs:52

`HasDialectEmitIR<'tokens, Language, LanguageOutput>` exists solely because the Rust trait solver cannot normalize `<W as HasDialectParser<'t>>::Output<T, L>` in impl-where-clauses without triggering E0275. The trait introduces a third lifetime parameter, a `Language` parameter, and a `LanguageOutput` parameter, creating a 5-dimensional dispatch space (`Self x 'tokens x Language x TypeOutput x LanguageOutput`). This is complex but the documentation is thorough about the rationale.

Given that the `ParseEmit<L>` trait (3 implementation paths) already provides the public-facing API, `HasDialectEmitIR` is effectively an implementation detail of the derive macro. The concern is that it is `pub` and could be accidentally depended upon.

**Alternative formalisms:**

| Approach | Conceptual complexity | Derive effort | API surface |
|----------|----------------------|---------------|-------------|
| Current witness trait (pub) | High (5 params) | Low (derive handles it) | Leaky |
| `pub(crate)` witness + sealed trait | Medium | Low | Clean |
| Defunctionalized callback (no trait, closure-only) | Low | Higher (manual plumbing) | Minimal |

**Suggested action:** Mark `HasDialectEmitIR` as `#[doc(hidden)]` or restrict visibility to `pub(crate)` if only the derive crate references it. The public API should be `ParseEmit<L>` only.

**References:** Yallop & White, "Lightweight Higher-Kinded Polymorphism" (defunctionalization as alternative to GATs).

## Strengths

- The `ParseEmit<L>` three-path design (derive, marker, manual) is a well-structured typeclass hierarchy that minimizes boilerplate while keeping full-control escape hatches.
- The two-pass pipeline parsing (`ParsePipelineText`) with header collection in pass 1 and body emission in pass 2 is a correct encoding of the standard forward-reference resolution problem, avoiding the need for fixpoint iteration.
- `DirectlyParsable` as a marker trait for identity `EmitIR` is an elegant way to avoid orphan-rule issues with blanket impls.
