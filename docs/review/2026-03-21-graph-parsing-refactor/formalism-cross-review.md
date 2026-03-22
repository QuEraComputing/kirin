# Formalism -- Cross-Review

## U1: Core IR

### Reviewed Findings

- [agree] Code-Quality P1: DiGraphBuilder vs UnGraphBuilder duplication -- This aligns exactly with my P2 finding on `DiGraphInfo`/`UnGraphInfo` structural near-duplication. I described the parametric `GraphInfo<L, D: EdgeType, Extra>` approach; Code Quality suggests `GraphBuilderCommon` helpers. Both are valid; the parametric approach is more principled (one type, not two plus helpers) but the helper approach is lower-risk. I would keep my severity at P2 since the duplication is a maintenance burden, not a correctness issue.
- [agree] Code-Quality P2: DiGraphInfo vs UnGraphInfo accessor duplication -- Same root cause as above. Subsumes into a single refactoring.
- [agree] Code-Quality P2: `new()` -> `build()` rename -- Clear convention violation. The 4 clippy suppressions are unnecessary if renamed.
- [severity-adjust] Code-Quality P3: `unit_cmp` on `Signature<T, C = ()>` -- Technically correct but should be P4/informational. The default `C = ()` comparison is semantically correct (all units are equal); the lint fires on a valid comparison. A `size_of` guard adds complexity for no behavioral gain.
- [agree] Ergonomics P1: `new()` convention violation -- Same finding as Code Quality P1, independently confirmed. Good signal that this is real friction.
- [agree] Ergonomics P2: Port placeholder lacks dedicated builder method -- Valid DX concern. Users must reach for `BuilderSSAKind::Unresolved(ResolutionInfo::Port(BuilderKey::Index(0)))` when `stage.port_ref().index(0)` would be natural by analogy with `block_argument()`.
- [agree] Ergonomics P2: DiGraph/UnGraph duplication -- Third independent confirmation across three reviewers.
- [agree] Dialect-Author P2: `DiGraphInfo`/`UnGraphInfo` duplication -- Fourth confirmation. Universal finding.
- [agree] Dialect-Author P2: `attach_nodes_to_ungraph` BFS duplication -- Novel finding I missed. The BFS logic duplication between `UnGraphBuilder::new` and `attach_nodes_to_ungraph` is a real maintenance risk.
- [agree] Soundness P0: `mem::zeroed()` on `SSAInfo<L>` -- This is the same finding as my P0. The Soundness reviewer and I arrived at the same conclusion independently with compatible analyses. Their "stale ID" attack vector and my "drop zeroed String" attack vector are complementary -- both demonstrate the unsoundness. The Soundness review's additional detail about `finalize_unchecked` creating zeroed *live* items (their P1) is a valuable separate finding.
- [agree] Soundness P1: `finalize_unchecked` zeroed live `SSAInfo` -- Distinct from the P0 (which is about deleted tombstones). This is about live items with zeroed types. My review folded both into a single P0; the Soundness review correctly separates them by attack vector.
- [severity-adjust] Soundness P3: `port_name`/`capture_name` debug-only assertion -- Agree on finding, but I would raise to P2. Silent name loss in release builds violates the principle of least surprise and is hard to debug. The ergonomics reviewer's port placeholder finding is related: if a user must already navigate a low-level API, silent failures compound the confusion.

### Low Priority Candidates

- Code-Quality P3: `language.rs` TODO comment -- Stale TODOs are noise but not actionable review findings.
- Ergonomics P3: High concept budget for graph operations -- Valid observation but inherent to the domain (graph IR is complex). Not actionable without redesigning the graph model.

### Cross-Cutting Insights

- The `mem::zeroed()` finding was independently flagged by both Soundness and Formalism reviewers with complementary attack vectors, which is strong evidence it should be the highest-priority fix.
- The DiGraph/UnGraph duplication was flagged by all four reviewers (Code Quality, Ergonomics, Dialect Author, Formalism). This unanimity suggests it should be bundled as a single refactoring task with the parametric `GraphInfo<L, D, Extra>` approach I described, which subsumes all four findings.

## U2: Parser Runtime

### Reviewed Findings

- [agree] Code-Quality P2: `port_list()`/`capture_list()` duplication -- Trivially correct. Same grammar, different label.
- [agree] Code-Quality P2: Statement-semicolon pattern repeated 5+ times -- Valid. A `statement_list(language)` helper is clean.
- [agree] Code-Quality P1: `parse_text.rs` at 978 lines -- Agree on decomposition need, but the suggested split into 3 files may not be ideal. The two-pass architecture couples pipeline parsing and statement parsing tightly; splitting by pass (pass1.rs, pass2.rs) might be more natural than splitting by trait.
- [agree] Ergonomics P2: Three ParseEmit paths create decision paralysis -- My formalism review identified the same structural issue from a different angle: `HasDialectEmitIR` is a 5-dimensional dispatch space that exists as an implementation detail. The ergonomics finding is the user-facing symptom of the same underlying complexity.
- [agree] Ergonomics P2: `ChumskyError` conflates parse and emit domains -- Valid. The name is misleading since `EmitError` has nothing to do with chumsky. `ParseAndEmitError` or `TextParseError` would be clearer.
- [severity-adjust] Ergonomics P1: EmitContext forward-reference mode invisible to users -- I would lower to P2. Forward references are needed only for graph bodies, which are an advanced feature. The derive handles this automatically; manual users are a small minority.
- [agree] Dialect-Author P1: No discoverable documentation for projection names -- Valid and cross-cuts with my finding about the format string lacking a formal grammar (U4 formalism P2). A documented grammar would implicitly document the projection vocabulary.
- [agree] Dialect-Author P2: Graph parser error messages lack context -- Valid but hard to fix within chumsky's error model. Lower practical impact than it seems.
- [agree] Dialect-Author P3: `port_list`/`capture_list` identical -- Third confirmation of Code Quality finding.
- [agree] Soundness P1: Duplicate SSA names silently shadow -- This is the same root issue I identified as my P1 (flat EmitContext without scoping). The Soundness reviewer focuses on the shadowing symptom; my formalism review identifies the root cause as missing scope-stack discipline. Both are correct and complementary.
- [severity-adjust] Soundness P2: `expect` panics in pipeline parse -- Agree on finding but the "concurrent mutation" attack vector is artificial. The `expect` calls guard post-validation invariants. I would lower to P3 (defense-in-depth improvement, not a real-world risk).
- [agree] Soundness P2: `expect` on first-pass function/symbol resolution -- Same assessment as above. Currently unreachable; defense-in-depth.
- [agree] Soundness P3: Duplicate block names silently shadow -- Same root cause as the SSA shadowing finding. A scope-stack approach (my suggestion) would fix both simultaneously.

### Low Priority Candidates

- Code-Quality P3: No `#[must_use]` on parser combinator return types -- Chumsky's own combinators already lack this; adding it to wrappers would be inconsistent with the library's style.
- Soundness P2/P2: The two `expect` findings on pipeline parse -- Currently unreachable through normal input. Defense-in-depth is good engineering but low priority relative to the shadow/scoping issues.

### Cross-Cutting Insights

- The SSA name shadowing (Soundness P1) and my flat-map scoping critique (Formalism P1) describe the same deficiency from different angles. Fixing the scope stack (my suggestion) would automatically prevent shadowing (their finding). This should be treated as a single high-priority item.
- The Dialect Author's finding about undiscoverable projection names connects directly to my U4 finding about the format string lacking a formal grammar. A single EBNF spec document would address both.

## U3: Derive Infrastructure

### Reviewed Findings

- [agree] Code-Quality P2: `#[allow(clippy::large_enum_variant)]` on `Data<L>` and `FieldData<L>` -- Correct findings but the suppression is justified. These are proc-macro-time types created once per compilation. Boxing would add allocation noise with no meaningful benefit.
- [false-positive] Code-Quality P2: FieldInfo accessor pairs could use a macro -- The explicit accessor form is more debuggable and IDE-friendly. A `field_accessor!` macro would harm readability for ~30 lines saved. The current code is better.
- [agree] Code-Quality P3: `field_iter_set.rs` repetitive generation -- Correct but inherent to the code generation domain.
- [severity-adjust] Code-Quality P2: `misc.rs` is a grab-bag module -- I would lower to P3. In a derive crate, utility functions like `to_camel_case` and `is_type` are called from many sites; splitting them adds navigation cost without improving cohesion. The 310-line size is reasonable for a utility module.
- [agree] Code-Quality P2: Manual Clone impls for FieldData/FieldInfo -- Worth investigating whether `#[derive(Clone)]` works now. If `Layout::ExtraFieldAttrs: Clone` is always a bound, the manual impls are 30 lines of unnecessary code.
- [agree] Ergonomics P2: Adding a new FieldCategory requires ~10 files -- This is the expression problem I identified in my U4 formalism review (P1). The ergonomics reviewer quantifies the practical cost; my review analyzes the theoretical structure.
- [severity-adjust] Ergonomics P1: 19 const declarations in generate.rs -- I would lower to P2. The registry-table approach is cleaner but the current pattern works and is mechanical. A typo produces a compile error (in user code, as noted), but the fix is straightforward. The real issue is the expression problem, not the boilerplate volume.
- [agree] Ergonomics P3: Layout trait 4 associated types defaulting to () -- My formalism review analyzed this as a profunctor and concluded the design is adequate. The ergonomics concern about needing to specify all 4 types when overriding one is valid but rare in practice (only 2 custom Layout impls exist).

### Low Priority Candidates

- Code-Quality P3: `field_iter_set.rs` repetitive generation -- Inherent to the domain; no practical fix without a meta-meta-template.
- Code-Quality P2: `large_enum_variant` suppressions -- Justified suppressions for compile-time-only types.

### Cross-Cutting Insights

- The Ergonomics P2 (new FieldCategory requires ~10 files) and my U4 formalism P1 (FieldCategory is a closed sum / expression problem) describe the same structural issue. The mitigation differs: the ergonomics reviewer wants a registry, I suggest `#[non_exhaustive]` with compile_error fallback arms. Both could be applied simultaneously.
- My assessment that `Layout`'s 4-parameter design is adequate (a correct profunctor) is reinforced by the ergonomics review's observation that `StandardLayout` suffices for most derives. The extension point complexity is justified by the rare cases that need it.

## U4: Parser/Printer Codegen

### Reviewed Findings

- [agree] Code-Quality P2: `field_kind.rs` `ast_type()` match arm duplication -- Correct, low impact. The DiGraph/UnGraph arms are near-identical.
- [agree] Code-Quality P2: `pretty_print/statement.rs` vs `parser/chain.rs` parallel dispatching -- This is the structural duality I noted in my strengths section: parse and print are dual operations over the same `FieldCategory` dispatch. The parallelism is inherent and should be documented, not eliminated.
- [agree] Code-Quality P2: `ValidationVisitor` with 7 tracking sets -- Valid complexity concern. Sub-struct grouping would improve readability.
- [severity-adjust] Code-Quality P1: `chain.rs` at 615 lines -- I would lower to P2. The file is large but it handles a single concern (parser chain generation). Splitting by field category would scatter related logic across files.
- [agree] Code-Quality P3: Unused `_ast_name`/`_type_params` parameters -- Either dead parameters or reserved for future use. Worth cleaning up.
- [agree] Ergonomics P2: Format string syntax not documented outside source -- Directly supports my P2 finding about the missing formal grammar. We arrive at the same recommendation (explicit grammar spec) from different perspectives.
- [agree] Ergonomics P2: `$keyword` vs `{.keyword}` migration error -- Nice DX improvement suggestion. Low priority.
- [severity-adjust] Ergonomics P3: Body projection completeness checking is strict -- I would keep at P3 or lower. The strictness is correct for roundtrip fidelity. A `#[kirin(no_captures)]` escape hatch would weaken the roundtrip guarantee and should only be added if there is real user demand.
- [agree] Ergonomics P1: RenderDispatch derive is undocumented -- Valid. A doc comment explaining the trait's role in pipeline printing would be a low-effort improvement.
- [agree] Soundness P1: Parser/printer asymmetry for split Signature projections -- This is a genuinely important finding. The split `{sig:inputs}`/`{sig:return}` projection reconstructs a `Signature` from two parsed halves, and any mismatch would break roundtrip. A dedicated roundtrip test for split signature projections is the correct mitigation.
- [agree] Soundness P3: `expect` panics in codegen on post-validation invariants -- Defense-in-depth recommendation. The `expect` calls guard post-validation invariants that should never be violated; replacing with `syn::Error` is good engineering but not urgent.
- [agree] Soundness P3: `expect` for `ir_path` in graph body projections -- Valid. Moving the check to the validation phase is the correct fix.

### Low Priority Candidates

- Code-Quality P3: Unused `_ast_name`/`_type_params` parameters -- Cosmetic cleanup.
- Ergonomics P3: Strict body projection completeness -- The strictness is a feature, not a bug.

### Cross-Cutting Insights

- The Soundness P1 (parser/printer asymmetry for Signature projections) is the most novel finding in U4 across all reviewers. My formalism review's observation that parse and print are "structurally dual" assumed this duality is maintained, but the Soundness reviewer identified a concrete case where reconstruction from projections could break it. This finding should be prioritized.
- My P2 (missing formal grammar) and the Ergonomics P2 (undocumented format syntax) converge: a single EBNF in the format.rs module doc comment addresses both. This was also connected to the U2 Dialect Author finding about undiscoverable projection names.

## U5: Output & Dialects

### Reviewed Findings

- [agree] Code-Quality P1 (x3): `print_digraph`/`print_ungraph` duplication with body-only variants -- Clean finding. The same body-rendering logic exists in both full and projected variants. Extracting `render_*_body_inner()` is straightforward.
- [agree] Code-Quality P1: FunctionBody vs Lambda interpreter duplication -- Valid. Both access `self.body.blocks(stage).next()` identically. A shared helper or blanket impl over a `HasRegionBody` trait would eliminate this.
- [agree] Code-Quality P2: `ir_render.rs` at 604 lines -- Agree with decomposition into graph/function/core rendering files.
- [agree] Code-Quality P2: Missing `#[must_use]` on `RenderBuilder` -- Good practice for builder patterns.
- [severity-adjust] Ergonomics P2: Five derives required per language enum -- I would lower to P3. The five derives are not boilerplate -- each generates distinct, useful code. A meta-derive `#[derive(Language)]` would hide the dialect author contract (parser, printer, interpreter all required) and make it harder to understand what is generated. The visual density is a one-line cost at the definition site.
- [severity-adjust] Ergonomics P2: `PrettyPrint` requires `L::Type: Display` on every method -- My formalism review analyzed this and concluded the recursive `L: PrettyPrint` bound is the standard encoding for mutually recursive pretty-printers. The `L::Type: Display` clause is redundant given `CompileTimeValue: Display`, so it could be moved to a supertrait. However, this is a minor ergonomic improvement, not a P2.
- [agree] Ergonomics P1: `PrettyPrint` vs `PrettyPrintExt` naming confusion -- Valid. The Ext trait pattern is standard Rust (cf. `FutureExt`, `StreamExt`) but the naming is indeed confusing for users who expect `.sprint()` to be on `PrettyPrint` itself.
- [agree] Ergonomics P3: `Lexical` vs `Lifted` distinction requires compiler knowledge -- Valid but inherent to the domain. The suggestion for a one-line recommendation in the doc is low-effort and helpful.
- [agree] Dialect-Author P2: `print_digraph`/`print_ungraph` share body patterns -- Fourth confirmation of the Code Quality duplication finding.
- [false-positive] Dialect-Author P3: `Bind` has no interpreter support -- `Bind` is intentionally unsupported in the interpreter because binding semantics require a separate lowering pass. Returning `Err(Unsupported)` is the correct behavior, not a missing feature. A doc comment explaining this would be good, but this is not a finding.
- [severity-adjust] Dialect-Author P1: `PrettyPrint` trait requires `L: PrettyPrint` bound -- Same finding as Ergonomics P2. I would set this at P3, not P1. The bound cascade is the standard cost of a recursive pretty-printer over a closed type family. The derive handles it; only manual implementors see the bounds.

### Low Priority Candidates

- Ergonomics P3: `Lexical` vs `Lifted` guidance -- A one-line doc improvement, minimal effort.
- My own U5 P2 on `Lexical`/`Lifted` isomorphism -- Theoretically interesting (row polymorphism) but with only 2 modes, the duplication is negligible. Would only matter if a third mode were added.

### Cross-Cutting Insights

- The print_digraph/print_ungraph body duplication (Code Quality P1) parallels the DiGraph/UnGraph builder duplication in U1. Both stem from the same root cause: the di/un graph types are not parameterized by edge directedness. The U1 `GraphInfo<L, D, Extra>` refactoring would propagate to U5's rendering, potentially resolving both duplication sites simultaneously.
- The FunctionBody/Lambda interpreter duplication (Code Quality P1) is a separate issue but follows the same pattern as my `Lexical`/`Lifted` isomorphism observation. Both could be addressed by extracting a `HasRegionBody` trait or shared sub-type.
- The `PrettyPrint` recursive bound was flagged by three reviewers (Ergonomics, Dialect Author, Formalism). My analysis confirms it is sound and standard. The actionable part is narrow: move the `L::Type: Display` where-clause to a supertrait bound to reduce ceremony for manual implementors.
