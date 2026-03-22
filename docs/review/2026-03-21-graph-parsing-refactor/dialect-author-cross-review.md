# Dialect Author -- Cross-Review

## U1: Core IR

### Reviewed Findings

- **[agree]** U1-formalism P0 `mem::zeroed()` on generic `SSAInfo<L>` -- This directly affects dialect authors. Our `L::Type` is the zeroed field, and non-trivial type lattices (enums, `String`-containing types) will produce UB. The `Option`-wrapping approach is the right fix because it requires no new bounds on `Dialect::Type`. Adding `Default` would be wrong -- `Placeholder` exists for exactly this purpose but should not leak into arena internals either.

- **[agree]** U1-formalism P2 `DiGraphInfo`/`UnGraphInfo` duplication -- Agree this is real duplication but from a dialect author perspective this is internal-only. Dialect authors never touch `DiGraphInfo` directly; they use the builder API. The builder dedup (U1-code-quality P1) is the one that matters more.

- **[agree]** U1-code-quality P2 `new()` -> `build()`/`finish()` rename -- Dialect authors building graph bodies call `.new()` expecting a constructor. The clippy suppression is the code telling us the name is wrong. Renaming to `.build()` is a trivial but high-value DX fix.

- **[agree]** U1-code-quality P1 DiGraphBuilder vs UnGraphBuilder duplication -- Agree with the finding but **severity-adjust down to P2** from a dialect author perspective. The duplicated internals do not affect the builder API surface. The risk is maintenance drift (a bug fixed in one but not the other), which is an internal concern.

- **[severity-adjust P3->P2]** U1-code-quality P3 stale TODO in language.rs -- Stale TODOs in core IR files erode trust for dialect authors reading the source to learn patterns. Minor but worth cleaning.

- **[agree]** U1-ergonomics P2 port placeholder lacks dedicated builder method -- This is the most impactful DX finding for graph-dialect authors. Requiring `BuilderSSAKind::Unresolved(ResolutionInfo::Port(BuilderKey::Index(0)))` to reference a port inside a graph node is unreasonable. A `stage.port_ref(0)` convenience is essential for any dialect that uses graph bodies.

- **[severity-adjust P3->P2]** U1-ergonomics P3 concept budget for graph operations -- From a dialect author adding graph-body support to a new dialect, the number of concepts (Port, PortParent, edge_count boundary, BuilderKey, ResolutionInfo) is genuinely high. This combines with the missing port placeholder method to make graphs the hardest part of dialect authoring by a wide margin.

- **[agree]** U1-soundness P0 `mem::zeroed()` UB through stale ID -- Same finding as formalism P0, confirming from a different angle. Dialect authors who save SSAValues across builder mutations (common during emit_ir) could trigger this. Agree with the severity.

- **[agree]** U1-soundness P1 `finalize_unchecked` zeroed live SSAInfo -- This is reachable through normal `with_builder` round-trips after parsing. A dialect author parsing text and then inspecting `.ty()` on forward-referenced SSAs would get a zeroed type. This is a real bug, not just a theoretical concern.

- **[agree]** U1-soundness P3 `port_name`/`capture_name` silent drop in release -- Dialect authors misusing the builder order lose names silently. The `debug_assert!` -> `assert!` promotion is the right fix. Builder ordering errors should always fail loudly.

- **[false-positive]** U1-code-quality P3 `unit_cmp` suppression -- This only fires when `C = ()`, which is the default constraint type for simple signatures. Dialect authors using `Signature<T>` (no constraints) will always hit `C = ()`. The suppression is correct because the comparison is intentionally generic. The suggested `size_of` guard adds complexity for zero benefit -- the comparison of `&()` is correct and optimizes to nothing.

- **[agree]** U1-code-quality P2 missing `#[must_use]` on builders -- Dialect authors silently dropping a `DiGraphBuilder` lose all the graph construction work. This is a real footgun.

### Low Priority Candidates

- U1-formalism P2 (DiGraphInfo/UnGraphInfo structural dedup) -- correct finding but invisible to dialect authors who only use the builder API.
- U1-code-quality P3 (`unit_cmp`) -- false-positive as argued above, but even if addressed it has zero dialect-author impact.

### Cross-Cutting Insights

- **Graph bodies are the hardest dialect-authoring surface.** The combination of missing port placeholder convenience (U1-ergo P2), high concept budget (U1-ergo P3), and silent builder misuse (U1-sound P3) makes graph-body dialects significantly harder to author than block/region dialects. A "graph dialect quickstart" example or a `GraphDialect` tutorial would help, but the port placeholder method is the mechanical fix that matters most.

- **`mem::zeroed()` affects the parse-then-inspect workflow.** Dialect authors commonly parse text, then inspect the resulting IR to verify their parser works. The zeroed-type bug (U1-sound P1) makes this workflow silently return garbage for forward-referenced SSAs, which is exactly the scenario that graph parsers produce.

---

## U2: Parser Runtime

### Reviewed Findings

- **[severity-adjust P1->P2]** U2-formalism P1 `EmitContext` flat map lacks scoping -- The finding is technically correct, but dialect authors do not interact with `EmitContext` scoping directly. The derive-generated `EmitIR` impls handle scope correctly because each `Region::emit_with` creates a fresh context. Manual `EmitIR` implementors could hit this, but that is path 3 (advanced). For the common derive path, this is not a dialect-author concern. Downgrade to P2 for dialect authors.

- **[agree]** U2-formalism P2 `HasDialectEmitIR` is pub but internal -- Dialect authors should never see or depend on this trait. Marking it `#[doc(hidden)]` is the right call. This reduces API surface noise when exploring the crate docs.

- **[agree]** U2-code-quality P2 `port_list()` vs `capture_list()` duplication -- Minor but these are the graph parser combinators that derive-generated code calls. If a dialect author needs to write a custom graph parser, they would encounter and potentially copy-paste this pattern. Low impact.

- **[severity-adjust P1->P2]** U2-code-quality P1 `parse_text.rs` at 978 lines -- Dialect authors never read this file. It is infrastructure. The decomposition would help maintainers but not dialect authors. Downgrade to P2 for this perspective.

- **[agree]** U2-ergonomics P2 three `ParseEmit` paths cause decision paralysis -- This is the single most confusing aspect of the parser for new dialect authors. The decision table suggestion is exactly right: "Use derive. If you cannot use derive and have no Block/Region fields, use `SimpleParseEmit`. Otherwise, implement manually." This should be in the module-level docs.

- **[severity-adjust P2->P1]** U2-ergonomics P1 `EmitContext` forward-reference mode invisible -- Upgrade for dialect authors. When writing a graph-body dialect, the author must call `set_relaxed_dominance(true)` but there is no documentation path that leads them there. The distinction between `resolve_ssa` (creates placeholder) and `lookup_ssa` (returns None) is critical for graph parsing correctness. Without documentation, authors will use `lookup_ssa`, get `None`, and be stuck.

- **[agree]** U2-ergonomics P2 `ChumskyError` name is confusing -- Dialect authors see this in their error types. The name suggests a chumsky-internal error when it actually covers the full parse+emit pipeline. `ParseAndEmitError` is a better name.

- **[agree]** U2-soundness P1 duplicate SSA names silently shadow -- Dialect authors writing test inputs or building DSLs on top of the text format will hit this. Typos that reuse `%x` produce silently wrong IR. This should be an error.

- **[severity-adjust P2->P3]** U2-soundness P2 `expect` panics in pipeline parse -- Only reachable through concurrent mutation or internal logic bugs, not through dialect author actions. Downgrade to P3 for this perspective.

- **[severity-adjust P2->P3]** U2-soundness P2 `expect` on first-pass function/symbol resolution -- Same reasoning. Internal logic, not dialect-author reachable.

- **[agree]** U2-soundness P3 duplicate block names silently shadow -- Same class as SSA shadowing. Dialect authors writing multi-block test inputs could hit this. The fix (check-before-insert) is trivial and should match SSA handling.

- **[false-positive]** U2-code-quality P2 `#[allow(dead_code)]` on `Header.stage/function` -- These fields document the grammar structure. The ergonomics reviewer correctly noted they are consumed via span-based re-parsing. The `#[expect]` suggestion is fine but calling this a finding overstates its importance. Dead-code in grammar structs is a standard pattern.

### Low Priority Candidates

- U2-code-quality P2 `#[allow(dead_code)]` on TestDialect -- test-only, zero dialect-author impact.
- U2-code-quality P3 missing `#[must_use]` on parser combinators -- chumsky's own types handle this; dialect authors use the derive, not raw combinators.
- U2-code-quality P2 statement-semicolon pattern duplication -- internal parser infrastructure.
- U2-ergonomics P3 `parse_ast` vs `parse_statement` error type inconsistency -- `parse_ast` is an advanced API; dialect authors use `parse_statement`.

### Cross-Cutting Insights

- **The forward-reference / relaxed-dominance API is the critical undocumented workflow for graph-dialect authors.** Combines with U1's graph builder complexity to make graph dialects the hardest authoring path. The `set_relaxed_dominance` + `resolve_ssa` vs `lookup_ssa` distinction needs explicit documentation aimed at dialect authors who are adding graph bodies.

- **The `ParseEmit` decision paralysis (U2-ergo P2) and the `HasDialectEmitIR` leaky abstraction (U2-form P2) are the same root issue.** The internal complexity of the parser trait hierarchy leaks through docs and error messages. Hiding `HasDialectEmitIR` and adding a decision table for `ParseEmit` would address both.

- **Duplicate SSA/block name shadowing (U2-sound P1, P3) is the most likely dialect-author bug.** Dialect authors write test inputs by hand constantly. A misspelled or copy-pasted SSA name producing silently wrong IR defeats the purpose of text-format testing.

---

## U5: Output & Dialects

### Reviewed Findings

- **[agree]** U5-formalism P2 `PrettyPrint` recursive `L: PrettyPrint` bound -- The formalism reviewer correctly concluded no change needed. From a dialect author perspective: the derive handles this transparently. Manual `PrettyPrint` impls require the bound, but it is always satisfiable. Non-issue.

- **[false-positive]** U5-formalism P2 `Lexical` vs `Lifted` isomorphism -- These are intentionally separate types representing different semantic choices (closures vs lifted functions). Merging them into a generic `Function<T, Mode>` would obscure the semantic distinction that dialect authors need to make at composition time. The 3-variant overlap is the cost of clear naming. The formalism reviewer acknowledged this ("acceptable given only two modes exist") -- I would go further and say it is the *correct* design.

- **[agree]** U5-code-quality P1 `print_ungraph()` vs `print_ungraph_body_only()` duplication -- Agree these should share a common inner renderer. Dialect authors writing custom `PrettyPrint` impls for graph types might copy-paste from these as examples, so the duplication could propagate.

- **[agree]** U5-code-quality P1 `print_digraph()` vs `print_digraph_body_only()` duplication -- Same as above, smaller scope.

- **[severity-adjust P1->P2]** U5-code-quality P1 `FunctionBody` vs `Lambda` interpreter duplication -- Dialect authors do not write interpreter impls for `FunctionBody`/`Lambda` (these are in kirin-function). The duplication is internal. The finding is correct for maintainers but P2 for dialect authors.

- **[agree]** U5-code-quality P2 `ir_render.rs` decomposition -- Agree but low dialect-author impact. This file is infrastructure that the derive calls into.

- **[agree]** U5-code-quality P2 missing `#[must_use]` on `RenderBuilder` -- Dialect authors using `.render()` who forget `.into_string()` lose their output silently. Real footgun, worth fixing.

- **[severity-adjust P2->P1]** U5-ergonomics P2 five derives required for every language enum -- This is the single biggest ergonomics pain point for dialect authors. Every language enum requires `Dialect, HasParser, PrettyPrint, Interpretable, SSACFGRegion` (and sometimes `CallSemantics`). A meta-derive `#[derive(Language)]` expanding to the standard set would significantly reduce the visual and cognitive burden. Upgrade to P1 for dialect authors.

- **[agree]** U5-ergonomics P2 `L::Type: Display` clause on every `PrettyPrint` method -- Since `CompileTimeValue` already requires `Display`, this bound is always satisfied. It is pure noise for manual `PrettyPrint` implementors. Moving it to a supertrait bound eliminates copy-paste for anyone writing manual impls.

- **[severity-adjust P1->P2]** U5-ergonomics P1 `PrettyPrint` vs `PrettyPrintExt` naming confusion -- The ergonomics reviewer rated this P1, but from a dialect author perspective: authors implement `PrettyPrint` (via derive) and call `.sprint()` (via `use kirin_prettyless::prelude::*`). The `Ext` trait is in the prelude and auto-imported. The naming is mildly confusing when reading docs, but not when actually writing code. Downgrade to P2.

- **[agree]** U5-ergonomics P3 `Lexical` vs `Lifted` distinction requires compiler knowledge -- The doc comment is good but a one-line recommendation ("Start with `Lexical`") would help newcomers.

- **[agree]** U5-code-quality P3 `Call::interpret` length -- Agree with the finding but low dialect-author impact since this is kirin-function internal code.

### Low Priority Candidates

- U5-formalism P2 (Lexical/Lifted isomorphism) -- false-positive as argued; the separation is intentional and correct.
- U5-code-quality P2 (ir_render.rs decomposition) -- infrastructure, not dialect-author facing.
- U5-code-quality P3 (Call::interpret length) -- internal to kirin-function.

### Cross-Cutting Insights

- **The five-derive requirement (U5-ergo P2, adjusted to P1) is the most visible friction in the dialect authoring contract.** Combined with `#[wraps]`, `#[callable]`, and `#[kirin(builders, type = T)]`, a new language enum requires 5 derives + 3 attributes. A `#[derive(Language)]` that expands to the standard set would reduce the "hello world" for dialect composition from intimidating to approachable.

- **Graph rendering duplication (U5-CQ P1s) and graph builder duplication (U1-CQ P1) share a root cause.** The graph subsystem was added with directed and undirected variants implemented independently. A unified refactoring pass over graph builders, info types, and renderers would address 4 findings across U1 and U5 simultaneously.

- **The `L::Type: Display` redundant bound (U5-ergo P2) is a papercut that multiplies.** Every manual `PrettyPrint` impl, every manual `HasParser` impl, every trait method that touches types -- all repeat this bound. Baking it into the `Dialect` supertrait (where `CompileTimeValue` already guarantees it) would eliminate the bound from dozens of signatures across the codebase.
