# Graph & Parsing Refactor — Triage Review Report

**Date:** 2026-03-21
**Scope:** 9 crates, ~34K lines — graph-IR-node, signature, function-parsing changes
**Reviewers:** Formalism (PL Theorist), Code Quality (Implementer), Ergonomics (Physicist), Dialect Author, Soundness Adversary
**Per-crate reports:** `docs/review/2026-03-21-graph-parsing-refactor/<unit>/`

---

## Executive Summary

The graph-IR and parsing refactor is architecturally sound — the Port/SSAKind extension, Signature type, template-based derive system, and single-lifetime parser design are well-engineered. Two P0 soundness issues require immediate attention: `mem::zeroed()` on generic arena types produces UB, and `EmitContext` flat-map scoping produces silently wrong IR for nested constructs. The most impactful DX improvement is documenting the format string DSL (flagged by 3 reviewers across 2 units). The DiGraph/UnGraph duplication (~195 lines recoverable) was unanimously flagged by all reviewers and should be a single parametric refactoring.

| Severity | Count | Key Theme |
|----------|-------|-----------|
| P0 | 2 | `mem::zeroed()` UB + `EmitContext` silent wrong IR |
| P1 | 3 | SSA name shadowing, zeroed live SSAInfo, format string docs |
| P2 | 14 | Graph dedup, builder DX, naming, file decomposition |
| P3 | 5 | Defense-in-depth, informational |

---

## P0 Findings

### P0-1. `mem::zeroed()` on generic `SSAInfo<L>` produces UB on drop
**Crate:** kirin-ir | **File:** `builder/stage_info.rs:222,262,272`
**Confidence:** confirmed (Formalism + Soundness independently)

Both `finalize()` and `finalize_unchecked()` use `unsafe { std::mem::zeroed() }` to tombstone deleted arena items. `SSAInfo<L>` contains `SmallVec<[Use; 2]>` and `L::Type` — for any heap-allocating `L::Type` (String, Vec, enums), zeroed bytes are invalid. When `Arena` drops, it drops all items including tombstones → UB.

The arena already has `try_map_live_option` (data.rs:109) that uses `Option<U>` with `None` tombstones. Switching to that API is the minimal fix.

**Interaction with U2:** Forward-reference parsing creates SSAs that may be deleted during resolution, triggering the tombstone path. This is a normal graph parsing flow.

**Action:** Replace `mem::zeroed()` with `Option`-wrapping in the SSA arena. Use `try_map_live_option`.
**References:** Rustonomicon "Working with Uninitialized Memory"; rust-lang/rust#66151

### P0-2. `EmitContext` flat-map scoping produces silently wrong IR
**Crate:** kirin-chumsky | **File:** `traits/emit_ir.rs:36-38`
**Confidence:** confirmed (Formalism P1 + Soundness elevated to P0)

`EmitContext` uses flat `FxHashMap<String, SSAValue>` with no scope push/pop. When nested blocks share SSA names (e.g., `%x` in outer and inner blocks), the inner binding permanently overwrites the outer. Under relaxed dominance mode (graph parsing), even wrong resolutions succeed silently — no error diagnostic.

**Severity dispute:** Soundness argues P0 (silently wrong IR). Ergonomics/Dialect Author argue P2 (parser generates unique names, only hand-written input affected). The Soundness case is stronger: relaxed dominance actively suppresses the error that would normally catch this.

**Action:** Add `push_scope()` / `pop_scope()` methods to `EmitContext` using `Vec<FxHashMap>` scope stack. Standard MLIR OpAsmParser pattern.
**References:** MLIR OpAsmParser SSA scope stack; Appel, "Modern Compiler Implementation," Ch. 5

---

## P1 Findings

### P1-1. Duplicate SSA names silently shadow earlier definitions
**Crate:** kirin-chumsky | **File:** `traits/emit_ir.rs:91-93`
**Confidence:** confirmed

`register_ssa` uses `HashMap::insert`, silently overwriting previous SSA mappings. Input with `%x = add ...; %x = mul ...; %y = use %x;` produces IR where `%y` references the second `%x` with no error. First `%x` becomes an orphan.

**Note:** Code Quality adjusted to P2 (parser generates unique names). Dialect Author and Ergonomics kept at P1 (hand-written text is a first-class use case). Resolved at P1 — users write test inputs by hand constantly.

**Action:** Check for existing entry before insert; return `EmitError::DuplicateSSA` on collision. Same fix for duplicate block names (emit_ir.rs:99-101).

### P1-2. `finalize_unchecked` creates zeroed live `SSAInfo` for type-less SSAs
**Crate:** kirin-ir | **File:** `builder/stage_info.rs:256-268`
**Confidence:** confirmed

`with_builder` round-trips through `finalize_unchecked`. Forward-reference SSAs created during parsing have `ty: None`. The resulting `SSAInfo<L>` gets a zeroed `L::Type`. Downstream `.ty()` returns zeroed value.

**Action:** Track unresolved SSAs through `with_builder` and either resolve or error. Part of the P0-1 fix.

### P1-3. Format string DSL undocumented / projection names undiscoverable
**Crate:** kirin-derive-chumsky + kirin-chumsky | **Files:** `format.rs`, `parsers/graphs.rs`
**Confidence:** confirmed (3 reviewers, 2 units — highest-convergence documentation finding)

The format string mini-language (`$keyword`, `{field}`, `{field:type}`, `{field:ports}`, `{:name}`, `{{`) has no user-facing reference. Projection names (`ports`, `captures`, `body`, `args`, `yields`, `name`, `inputs`, `return`) are implicit in codegen. Dialect authors must read source or guess.

**Action:** Add EBNF grammar as doc comment in `format.rs`:
```
format  ::= element*
element ::= '{{' | '}}' | '$' IDENT | '{:' projection '}' | '{' field_ref '}' | token+
field_ref ::= (IDENT | INT) (':' option)?
option  ::= 'name' | 'type' | 'ports' | 'captures' | 'args' | 'body' | 'inputs' | 'return'
projection ::= 'name'
```

---

## P2 Findings

### P2-1. DiGraph/UnGraph duplication (~195 lines recoverable)
**Crate:** kirin-ir | **Files:** `node/digraph.rs`, `node/ungraph.rs`, `builder/digraph.rs`, `builder/ungraph.rs`, `builder/stage_info.rs:381`
**Confidence:** confirmed (all 5 reviewers flagged — unanimous)

`DiGraphInfo<L>` and `UnGraphInfo<L>` differ only in `petgraph::Directed` vs `Undirected` and one field (`yields` vs `edge_statements`). Builders share ~100 lines of identical port allocation, name-to-index maps, and SSA replacement logic. `attach_nodes_to_ungraph` duplicates ~60 lines of BFS logic.

**Action:** Extract `GraphInfo<L, D: EdgeType, Extra>` parameterized by directedness. Refactor shared builder logic into `GraphBuilderBase<L, D>`. This also enables merging `FieldCategory::DiGraph`/`FieldCategory::UnGraph` → `FieldCategory::Graph(Directedness)` in derive infrastructure, saving ~20 additional lines in field_kind.rs.

### P2-2. Builder `new()` → `build()` rename (eliminates 4 clippy suppressions)
**Crate:** kirin-ir | **Files:** `builder/digraph.rs:84`, `ungraph.rs:77`, `block.rs:93`, `region.rs:31`
**Confidence:** confirmed (Code Quality + Ergonomics independently)

All builders use `new(self) -> Id` pattern requiring `#[allow(clippy::wrong_self_convention)]`. Rename to `build()` or `finish()` eliminates all 4 suppressions.

### P2-3. Port placeholder lacks dedicated builder method
**Crate:** kirin-ir | **File:** `builder/` (missing)
**Confidence:** likely (Ergonomics + Dialect Author)

Users must construct `BuilderSSAKind::Unresolved(ResolutionInfo::Port(BuilderKey::Index(0)))` to reference graph ports. Blocks have `stage.block_argument().index(0)` but graphs have no analogous `stage.port_ref(0)`.

**Action:** Add `port_ref(idx)` and `capture_ref(idx)` convenience methods to `BuilderStageInfo`.

### P2-4. `port_name`/`capture_name` silently ignored in release builds
**Crate:** kirin-ir | **Files:** `builder/digraph.rs:48,64`, `builder/ungraph.rs:45,59`
**Confidence:** likely (Soundness + Ergonomics + Formalism elevated from P3)

`debug_assert!` guards for builder call ordering. In release, calling `port_name("x")` before `port()` silently drops the name.

**Action:** Promote to `assert!` or return `Result` for builder ordering errors.

### P2-5. Parser/printer asymmetry risk for split Signature projections
**Crate:** kirin-derive-chumsky | **File:** `field_kind.rs:210-224`
**Confidence:** likely (Soundness P1, Ergonomics adjusted to P2)

Split `{sig:inputs}` + `{sig:return}` produces two separate parsed values that must be reassembled into a Signature. Reconstruction correctness is not validated.

**Action:** Add a roundtrip test specifically for split signature projections.

### P2-6. `expect` for `ir_path` in graph body projections (user-triggerable panic)
**Crate:** kirin-derive-chumsky | **File:** `field_kind.rs:297,328`
**Confidence:** confirmed (Soundness P3, Ergonomics elevated to P2)

A misconfigured `#[kirin(crate = ...)]` on a graph-containing type outside the kirin workspace causes a proc-macro panic with an unhelpful message.

**Action:** Validate `ir_path` presence during the validation phase for types with graph body projections.

### P2-7. `HasDialectEmitIR` is pub but should be hidden
**Crate:** kirin-chumsky | **File:** `traits/has_dialect_emit_ir.rs:52`
**Confidence:** likely

Implementation detail of the derive macro. Should be `#[doc(hidden)]` or `pub(crate)`.

### P2-8. print_digraph/ungraph body duplication with body_only variants (~55 lines)
**Crate:** kirin-prettyless | **File:** `document/ir_render.rs:192-302,469-543`
**Confidence:** confirmed (Code Quality + Dialect Author)

Extract `render_digraph_body_inner()` and `render_ungraph_body_inner()` helpers. Coupled to P2-1 graph unification.

### P2-9. FunctionBody/Lambda interpreter duplication (~40 lines)
**Crate:** kirin-function | **File:** `interpret_impl.rs:9-78`
**Confidence:** confirmed

Extract `HasRegionBody` trait with blanket `SSACFGRegion` and `Interpretable` impls.

### P2-10. Missing `#[must_use]` on builder types
**Crate:** kirin-ir + kirin-prettyless | **Files:** `builder/*.rs`, `traits.rs`
**Confidence:** likely

`DiGraphBuilder`, `UnGraphBuilder`, `BlockBuilder`, `RegionBuilder`, `RenderBuilder` — all silently discard work if dropped.

### P2-11. `parse_text.rs` at 978 lines — decomposition
**Crate:** kirin-chumsky | **File:** `function_text/parse_text.rs`
**Confidence:** likely

Split into `parse_pipeline.rs`, `parse_statement.rs`, and `lookup.rs` (or by pass: `pass1.rs`, `pass2.rs`).

### P2-12. Three ParseEmit paths need decision table
**Crate:** kirin-chumsky | **File:** `traits/parse_emit.rs:62-80`
**Confidence:** confirmed

Add a decision table: "Use derive. No Block/Region + no recursion? → `SimpleParseEmit`. Custom logic? → manual."

### P2-13. `ChumskyError` name is misleading
**Crate:** kirin-chumsky | **File:** `traits/parse_emit.rs:11-16`
**Confidence:** likely

Wraps non-chumsky `EmitError`. Rename to `ParseAndEmitError` or `TextParseError`.

### P2-14. RenderDispatch derive undocumented
**Crate:** kirin-derive-prettyless | **File:** `lib.rs:15`
**Confidence:** confirmed

No doc comment explaining purpose, usage, or requirements.

---

## P3 Findings

### P3-1. `expect` calls in parser/codegen should be `syn::Error` / `map_err`
**Crates:** kirin-chumsky, kirin-derive-chumsky | **Files:** `parse_text.rs:377,432`, `chain.rs:52`, `statement.rs:190,192`
**Confidence:** confirmed

Systematic defense-in-depth improvement. `expect` in proc-macro context produces unhelpful ICE-style panics.

### P3-2. Duplicate block names silently shadow
**Crate:** kirin-chumsky | **File:** `traits/emit_ir.rs:99-101`
**Confidence:** likely

Same pattern as P1-1. Fix alongside SSA name shadowing.

### P3-3. `FieldCategory` closed enum — expression problem
**Crate:** kirin-derive-toolkit | **File:** `ir/fields/info.rs`
**Confidence:** likely (Formalism P1, cross-review consensus P2-P3)

Adding a new category requires ~10 files across 4 crates. Acceptable given rarity (3 additions ever). Note: if P2-1 graph unification happens, `DiGraph`/`UnGraph` categories could merge.

### P3-4. FieldIterConfig registry boilerplate in generate.rs
**Crate:** kirin-derive-ir | **File:** `generate.rs:22-184`
**Confidence:** confirmed (Ergonomics)

19 const declarations with identical structure. A declarative macro would save ~100 lines.

### P3-5. Lexical vs Lifted guidance needed in doc
**Crate:** kirin-function | **File:** `lib.rs:1-16`
**Confidence:** uncertain

Add one-line recommendation: "Start with `Lexical` unless you are writing a lowering pass."

---

## Cross-Cutting Themes

### 1. Silent failures are the top DX risk (P0-2, P1-1, P2-4, P2-10)
Duplicate SSA names, silently dropped port names, and silently dropped builders share the same failure mode: user does something wrong and gets no feedback. Address as a class.

### 2. Graph subsystem duplication spans U1 + U5 (P2-1, P2-8)
Graph info types, builders, BFS logic, and renderers all duplicate between directed/undirected. A single `GraphInfo<L, D, Extra>` refactoring resolves findings in both units simultaneously. ~250 total lines recoverable.

### 3. Format string DSL documentation is the highest-convergence gap (P1-3)
Three independent reviewers across two units. A single EBNF + projection table addresses all.

### 4. `mem::zeroed()` interacts with graph parsing (P0-1, P1-2)
Forward-reference SSAs created during graph parsing are later deleted, hitting the zeroed tombstone path. The P0 and P1-2 should be fixed together.

### 5. Reference implementation quality (kirin-function) matters (P2-9)
Dialect authors study kirin-function to learn patterns. Duplication in its interpreter sets a bad example.

---

## Architectural Strengths

1. **Port/SSAKind extension** cleanly unifies directed and undirected graph boundaries under SSA
2. **Signature<T, C>** is parametrically polymorphic with clean typeclass dispatch via `SignatureSemantics`
3. **Template system** (`TraitImplTemplate`, factory methods) provides principled composable codegen
4. **Single-lifetime `HasParser<'t>`** — clean simplification from old two-lifetime system
5. **ParseEmit three-path design** (derive, marker, manual) balances boilerplate vs control
6. **Format string validation** catches roundtrip-breaking errors at compile time
7. **Two-pass pipeline parsing** correctly solves forward-reference resolution
8. **Dialect supertrait bundle** with `for<'a>` HRTB avoids n-squared trait explosion

---

## Follow-Up Actions (Priority Order)

### Quick Wins (< 30 min each)
1. P2-2: Rename builder `new()` → `build()` — kirin-ir builders
2. P2-10: Add `#[must_use]` on builder types — kirin-ir + kirin-prettyless
3. P2-4: Promote `port_name`/`capture_name` guards to `assert!`
4. P2-7: Mark `HasDialectEmitIR` as `#[doc(hidden)]`
5. P2-14: Add doc comment to `#[derive(RenderDispatch)]`
6. P2-13: Rename `ChumskyError` → `TextParseError`
7. P1-1: Add duplicate-name check in `register_ssa` / `register_block`

### Moderate Effort (1-3 hours each)
8. P0-1: Replace `mem::zeroed()` with `Option`-wrapping in SSA arena
9. P0-2: Add scope stack to `EmitContext`
10. P1-3: Write format string EBNF grammar + projection table doc
11. P2-3: Add `port_ref()`/`capture_ref()` builder convenience methods
12. P2-5: Add roundtrip test for split signature projections
13. P2-6: Validate `ir_path` in validation phase for graph body projections
14. P2-9: Extract `HasRegionBody` trait for FunctionBody/Lambda
15. P2-12: Add ParseEmit decision table to docs

### Design Work (half-day+)
16. P2-1 + P2-8: Unify DiGraph/UnGraph into `GraphInfo<L, D, Extra>` — spans kirin-ir, kirin-prettyless, kirin-derive-toolkit, kirin-derive-chumsky
17. P2-11: Decompose `parse_text.rs` (978 lines)

### Documentation
18. P3-5: Add Lexical vs Lifted guidance to kirin-function docs
19. P3-4: Registry macro for FieldIterConfig declarations

---

## Filtered Findings

<details>
<summary>8 findings filtered</summary>

- U1 Code-Quality P3: `unit_cmp` on Signature — semantically correct comparison, lint is noise
- U1 Code-Quality P3: stale TODO in language.rs — informational only
- U1 Ergonomics P3: high concept budget for graphs — inherent to domain
- U3 Code-Quality P2: FieldInfo accessor pairs could use macro — explicit form is more debuggable (Formalism false-positive)
- U3 Formalism P2: Template `Vec<TokenStream>` — reviewer concluded no change needed
- U3 Formalism P2: Layout 4-parameter family — reviewer concluded adequate
- U5 Formalism P2: Lexical/Lifted isomorphism — intentional design, only 2 modes
- U5 Dialect Author P3: `Bind` unsupported in interpreter — intentional (requires lowering pass)
</details>
