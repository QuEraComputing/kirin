# Code Quality -- Cross-Review

## U1: Core IR

### Reviewed Findings

- **[agree] Formalism P0 -- `mem::zeroed()` on generic `SSAInfo<L>`**: Fully agree with both the Formalism and Soundness reviewers. This is genuine UB through a safe API path. Soundness reviewer's attack scenario (stale ID after placeholder resolution) is realistic and strengthens the Formalism reviewer's P0 classification. The `Option`-wrapping approach from Formalism is the cleanest fix; the `Default` bound approach is insufficient because it does not address the deleted-item-still-readable issue that Soundness identifies.

- **[agree] Soundness P1 -- `finalize_unchecked` creates zeroed live `SSAInfo`**: Agree. This is strictly worse than the deleted-item case because these are *live* items. The reachability through `with_builder` + forward references is a normal code path.

- **[agree] Formalism P2 / Ergonomics P2 / Dialect Author P2 -- DiGraph/UnGraph duplication**: All three reviewers independently found this. I flagged the same duplication in my initial review (~100 lines in builders, ~40 in Info structs). The convergence across four reviewers confirms this is the highest-priority dedup target in U1.

- **[agree] Ergonomics P1 -- `new()` method name on builders**: Agree. My initial review found the same `#[allow(clippy::wrong_self_convention)]` suppressions across all four builders. Renaming to `build()` eliminates the suppressions with zero semantic change.

- **[agree] Ergonomics P2 -- Port placeholder lacks dedicated builder method**: Valid DX issue. Users must construct `BuilderSSAKind::Unresolved(ResolutionInfo::Port(BuilderKey::Index(0)))` manually. A `stage.port_ref().index(0)` convenience method would match the `block_argument()` pattern.

- **[severity-adjust P3->informational] Soundness P3 -- `port_name`/`capture_name` debug_assert**: The Soundness reviewer is correct that the name is silently lost in release, but the `if let Some(last)` guard prevents a crash. The builder pattern naturally chains `.port(ty).port_name("x")`, and misordering is uncommon. Promoting to `assert!` is fine but low priority.

- **[false-positive] Ergonomics P3 -- concept budget "high"**: The concept count (8 concepts) is reasonable for a graph IR system. The reviewer notes it, but the concepts are inherent to the domain. Not actionable.

- **[agree] Dialect Author P2 -- `attach_nodes_to_ungraph` duplicates BFS logic**: Confirmed. This is a third site of graph-builder duplication I did not catch in my initial review. Strengthens the case for a shared `GraphBuilder<L, D>`.

### Formalism-Informed Duplication

- **`GraphInfo<L, D: EdgeType, Extra>` (Formalism P2)** -- eliminates duplication at `node/digraph.rs:19-91` and `node/ungraph.rs:19-91`. The structs differ in one field (`yields: Vec<SSAValue>` vs `edge_statements: Vec<Statement>`) and the petgraph directedness parameter. A generic `GraphInfo<L, D: EdgeType, Extra>` with the divergent field as `Extra` would unify both structs and all shared accessor methods. ~35 lines saved in the Info structs. Additionally, the shared builder logic (port allocation at `builder/digraph.rs:89-132` = `builder/ungraph.rs:82-124`, and replacement resolution at `builder/digraph.rs:134-183` = `builder/ungraph.rs:126-181`) would collapse into a generic `GraphBuilderBase<L, D>`. ~100 lines saved in builders. The `attach_nodes_to_ungraph` BFS duplication in `builder/stage_info.rs:381` adds another ~60 lines recoverable. **Total: ~195 lines saved, complexity: medium** (requires parameterizing `PortParent` and `StatementParent` by graph kind, plus updating the derive infrastructure's `FieldCategory` to handle the unified type).

### Cross-Cutting Insights

- The `mem::zeroed()` finding is the single highest-severity issue across the entire review. Both Formalism and Soundness converge on it independently, with complementary attack vectors (generic type invalidity vs stale-ID reachability). Fix this first.
- Graph duplication is the single most-confirmed finding: 4 of 5 reviewers flagged it independently. It spans Info structs, builders, and `stage_info.rs` BFS logic.

---

## U2: Parser Runtime

### Reviewed Findings

- **[agree] Formalism P1 -- `EmitContext` flat map lacks scoping**: Correct. The flat `FxHashMap` allows inner blocks to shadow outer SSA names permanently. For current usage (single function bodies, graph bodies with relaxed dominance), this has not caused bugs because the two-pass pipeline parser creates a fresh `EmitContext` per function. However, the Formalism reviewer is right that it is fragile for nested regions.

- **[severity-adjust P1->P2] Soundness P1 -- Duplicate SSA names silently shadow**: Technically correct, but the SSA text format convention uses unique numbered names (`%0`, `%1`, ...), and the parser itself generates unique names. Shadowing requires hand-crafted duplicate input. Still worth fixing (check-before-insert), but not P1 severity in practice because the parser is the primary producer of SSA names.

- **[agree] Ergonomics P2 -- Three ParseEmit paths create decision paralysis**: Valid. A decision table in the doc comment would cost 5 lines and eliminate the confusion.

- **[agree] Ergonomics P2 -- ChumskyError conflates parse and emit domains**: The name is misleading. The enum is sound but the name `ChumskyError` for a type that wraps non-chumsky `EmitError` is confusing.

- **[agree] Ergonomics P1 -- EmitContext forward-reference mode is invisible**: Valid. `set_relaxed_dominance(true)` is the only API for graph body forward references but is undocumented at the call site.

- **[agree] Dialect Author P1 -- No discoverable documentation for projection names**: Valid. The projection vocabulary (`ports`, `captures`, `body`, `args`, `yields`, `name`) is implicit in codegen. A compile-time error for misspelled projections would be ideal; failing that, a doc comment listing valid projections.

- **[agree] Dialect Author P3 -- `port_list` and `capture_list` identical**: My initial review found the same 12-line duplication. Trivially collapsible.

- **[severity-adjust P2->P3] Soundness P2 -- `expect` panics in pipeline parse**: The reviewer acknowledges this requires either concurrent mutation or internal logic bugs. Not reachable through normal text input. Defense-in-depth replacement is good practice but low priority.

- **[agree] Soundness P3 -- Duplicate block names silently shadow**: Same pattern as SSA names. Worth fixing alongside the SSA shadow fix.

### Formalism-Informed Duplication

- **Scope stack for `EmitContext` (Formalism P1)** -- eliminates the need for the ad-hoc shadowing workarounds I found in my initial review. Currently, `EmitContext` at `traits/emit_ir.rs:37-38` uses flat maps. If a scope stack (`Vec<FxHashMap>`) were introduced, the `register_ssa` and `register_block` methods would naturally prevent cross-scope shadowing, eliminating the duplicate-name issue (Soundness P1, P3) without needing separate duplicate-detection logic. ~0 lines saved directly (it replaces one HashMap with a Vec of HashMaps), but eliminates the need for ~10 lines of future duplicate-checking code that would otherwise be needed. **Net effect: prevents 2 bugs structurally, complexity: low** (add `push_scope()`/`pop_scope()` methods, change lookup to iterate from top of stack).

- **`HasDialectEmitIR` visibility restriction (Formalism P2)** -- no direct duplication elimination, but marking it `#[doc(hidden)]` or `pub(crate)` reduces API surface. Formalism reviewer's defunctionalized callback alternative is not worth the derive effort increase. **No lines saved, complexity: low** (visibility change only).

### Cross-Cutting Insights

- The Formalism scope-stack proposal and Soundness duplicate-name findings are two sides of the same coin: both identify that `EmitContext`'s flat namespace is insufficient for nested IR. The scope stack solves both at once.
- The `port_list`/`capture_list` duplication (Dialect Author P3) corroborates my initial review finding and is the easiest fix in U2.

---

## U3: Derive Infrastructure

### Reviewed Findings

- **[agree] Ergonomics P2 -- Adding a FieldCategory requires ~10 files across 4+ crates**: Correct. This is the expression problem applied to IR body types. The current closed-enum approach is justified given the rarity of new categories (3 additions total), but the coordination cost is real.

- **[agree] Ergonomics P1 -- 19 const declarations in generate.rs**: Correct. The `FieldIterConfig` / `BoolPropertyConfig` pattern is highly repetitive. A registry macro or declarative table would reduce error surface. My initial review did not flag this specifically but it aligns with the `generate.rs` being 70% boilerplate.

- **[severity-adjust P2->P3] Formalism P2 -- Layout trait 4-parameter family could be simplified**: The Formalism reviewer themselves conclude "no structural change required." The `extra_statement_attrs_from_input` bridge is pragmatically correct. The profunctor framing is intellectually interesting but not actionable.

- **[false-positive] Formalism P2 -- Template `Vec<TokenStream>` vs document algebra**: The Formalism reviewer themselves conclude "no change needed." Proc-macro output is inherently flat token concatenation. The structured algebra adds complexity without benefit.

- **[agree] Ergonomics P3 -- Layout 4 associated types all defaulting to ()**: Valid but low priority. `StandardLayout` covers >90% of cases. The partial-override pattern would add infrastructure for a marginal benefit.

### Formalism-Informed Duplication

- **Registry macro for `FieldIterConfig` declarations (Ergonomics P1, informed by Formalism's "profunctor over levels" framing)** -- the Formalism reviewer's observation that `Layout` maps parse-source levels to attribute types suggests a similar registry pattern for `FieldIterConfig`. Currently, `crates/kirin-derive-ir/src/generate.rs:22-184` contains 14 `FieldIterConfig` constants and 5 `BoolPropertyConfig` constants with identical structure. A declarative macro `field_iter!(HasBlocks, blocks, Block, BlocksMut, blocks_mut)` would generate each config from a single line. ~100 lines saved in `generate.rs`, plus eliminating string-based typo risk. **~100 lines saved, complexity: low** (macro is purely declarative, no trait changes needed).

### Cross-Cutting Insights

- The expression problem for `FieldCategory` (Ergonomics P2) interacts with the Formalism reviewer's note about `#[non_exhaustive]` from U4. Adding `#[non_exhaustive]` to `FieldCategory` would make the coordination cost explicit at compile time rather than relying on match exhaustiveness to catch omissions.

---

## U4: Parser/Printer Codegen

### Reviewed Findings

- **[agree] Formalism P1 -- `FieldCategory` closed enum expression problem**: Correct analysis, correct conclusion. The closed enum is acceptable given the rarity of new categories. The `#[non_exhaustive]` suggestion is a good defensive measure.

- **[agree] Formalism P2 -- Format string DSL lacks formal grammar**: Valid. The implicit grammar-as-parser is sufficient for correctness but poor for documentation. Adding an EBNF comment costs 10 lines and provides a reference for validation rule authors.

- **[agree] Ergonomics P2 -- Format string syntax undocumented outside source**: Same finding as Dialect Author P1 in U2 (projection names undiscoverable). These two findings should be addressed together with a single documentation table.

- **[agree] Soundness P1 -- Parser/printer asymmetry for split Signature projections**: The concern about `{sig:inputs}` and `{sig:return}` producing separate parsed values that must be reassembled is valid. A roundtrip test specifically targeting split signature projections is the right mitigation.

- **[agree] Ergonomics P1 -- RenderDispatch derive undocumented**: Valid. The derive macro has no doc comment explaining its purpose or requirements.

- **[severity-adjust P3->informational] Ergonomics P3 -- Body projection completeness checking is strict**: The strictness is correct for roundtrip fidelity. A `#[kirin(no_captures)]` escape hatch would introduce a second way to express "no captures" (empty capture list vs annotation), increasing conceptual overhead. The current behavior (require the projection, allow empty) is clear.

- **[severity-adjust P3->informational] Soundness P3 -- `expect` in codegen**: These are post-validation invariants. The `expect` messages are clear enough for debugging. Converting to `syn::Error` is nice-to-have but not blocking.

- **[agree] Soundness P3 -- `expect` for ir_path in graph body projections**: This is more concerning than the general codegen expects because it is reachable through normal usage (misconfigured crate path). Validating `ir_path` presence during the validation phase is the right fix.

### Formalism-Informed Duplication

- **Formal grammar comment for format string DSL (Formalism P2)** -- does not directly eliminate code duplication, but provides the reference needed to verify that `parser_expr` and `print_expr` in `field_kind.rs` are symmetric. Currently, verifying roundtrip correctness requires mentally reconstructing the grammar from two separate match arms (parser at `field_kind.rs:120-270`, printer at `field_kind.rs:280-400`). With an explicit grammar, these two match arms could be verified against a common specification. **No lines saved, but reduces the mental overhead of maintaining parallel category dispatching** (my initial P2 finding about `chain.rs` vs `statement.rs` parallel structure).

- **`FieldCategory` `#[non_exhaustive]` (Formalism P1)** -- would not save lines but would make the expression-problem cost explicit, converting silent exhaustiveness failures (when a new category is added) into compile errors at downstream match sites. This directly addresses my initial review concern about the `ast_type()` match having several near-identical arms for DiGraph/UnGraph. If a `GraphInfo<L, D>` unification from U1 happens, `FieldCategory::DiGraph` and `FieldCategory::UnGraph` could merge into `FieldCategory::Graph(Directedness)`, eliminating ~20 lines of duplicated match arms across `ast_type`, `parser_expr`, and `print_expr`. **~20 lines saved across field_kind.rs, complexity: medium** (requires U1 unification first).

### Cross-Cutting Insights

- The Soundness P1 finding about Signature projection asymmetry is the most actionable finding in U4. A targeted roundtrip test is cheap and high-value.
- The format string documentation gap (Formalism P2, Ergonomics P2, Dialect Author P1 from U2) is flagged by three reviewers across two units. This is the highest-convergence documentation issue in the entire review.

---

## U5: Output & Dialects

### Reviewed Findings

- **[agree] Formalism P2 -- `PrettyPrint` recursive `L: PrettyPrint` bound**: The Formalism reviewer correctly identifies this as sound but all-or-nothing. Their own conclusion ("no change needed") is correct. The recursive bound is the standard encoding.

- **[agree] Formalism P2 -- `Lexical` vs `Lifted` isomorphic modulo one variant**: Correct analysis. The two enums share 3 of 4 variants. The Formalism reviewer's own conclusion ("acceptable given only two modes") is correct. A `Common<T>` sub-enum extraction is only worthwhile if a third mode appears.

- **[agree] Ergonomics P2 -- Five derives required per language enum**: Valid DX concern. A `#[derive(Language)]` meta-derive would reduce visual density. However, this is a one-time cost per language enum definition and the explicit derives are self-documenting. Severity is appropriate at P2.

- **[severity-adjust P2->P3] Ergonomics P2 -- `L::Type: Display` clause on every PrettyPrint method**: The reviewer notes that `CompileTimeValue` already requires `Display`, so the bound is always satisfied. However, removing it from the method signature would require a supertrait bound change, which touches all implementors. The bound is redundant but harmless. Lower priority than stated.

- **[agree] Ergonomics P1 -- PrettyPrint vs PrettyPrintExt naming confusion**: Valid. The naming does not communicate that `PrettyPrintExt` is the call-site trait. However, this is the standard Rust ext-trait pattern (e.g., `Iterator` vs `IteratorExt`, `Future` vs `FutureExt`). The pattern is well-established; the issue is discoverability for newcomers rather than a naming error.

- **[agree] Dialect Author P2 -- `print_digraph`/`print_ungraph` share header/body patterns**: My initial review found the same duplication. ~55 lines recoverable across digraph and ungraph rendering.

- **[agree] Dialect Author P3 -- `Bind` has no interpreter support**: Correct. `Err(Unsupported)` at runtime is surprising. A `#[deprecated]` or prominent doc comment would help.

- **[agree] Dialect Author P1 -- `PrettyPrint` trait requires `L: PrettyPrint` on every method**: Same finding as Ergonomics P2 (redundant Display bound). Both reviewers flag it independently, confirming it is a real ergonomics friction point for manual implementors.

### Formalism-Informed Duplication

- **`Common<T>` sub-enum for shared `FunctionBody`/`Call`/`Return` variants (Formalism P2)** -- eliminates duplication at `kirin-function/src/lib.rs` where `Lexical` and `Lifted` both wrap `FunctionBody`, `Call`, and `Return` with identical `#[wraps]` annotations. More significantly, in `interpret_impl.rs:9-78`, the `SSACFGRegion` and `Interpretable` impls for `FunctionBody` and `Lambda` are token-for-token identical (both access `self.body.blocks(stage).next()`). If `FunctionBody` and `Lambda` both implemented a shared `HasRegionBody` trait, a single blanket impl would replace both. **~40 lines saved in interpret_impl.rs, ~6 lines saved in lib.rs enum definitions. Total: ~46 lines saved, complexity: low** (trait + blanket impl, no changes to derive infrastructure).

- **Unified graph body rendering helper (Dialect Author P2, informed by Formalism's parametricity observation from U1)** -- if `GraphInfo<L, D, Extra>` unification from U1 is adopted, the printer could have a single `print_graph<D: EdgeType>` method instead of separate `print_digraph`/`print_ungraph`. The body rendering logic at `ir_render.rs:210-228` (digraph body) and `ir_render.rs:269-302` (ungraph body) would be parameterized by a `RenderBody` trait with `digraph_body` and `ungraph_body` as two impls. The `_body_only` variants at `ir_render.rs:469-543` would collapse similarly. **~55 lines saved in ir_render.rs, complexity: medium** (requires U1 unification, plus a trait for the divergent body rendering logic).

### Cross-Cutting Insights

- The `FunctionBody`/`Lambda` interpreter duplication (my initial P1 finding, confirmed by the Formalism reviewer's isomorphism observation) is the easiest high-value fix in U5. A `HasRegionBody` trait with blanket `SSACFGRegion` and `Interpretable` impls would eliminate 40 lines with no downstream changes.
- The graph rendering duplication in `ir_render.rs` is directly coupled to the U1 `GraphInfo` unification. If U1 is addressed, U5 printing follows naturally. These should be planned as a single refactor.
