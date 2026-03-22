# Ergonomics -- Cross-Review

## U1: Core IR

### Reviewed Findings

- **[agree]** Formalism P0 / Soundness P0+P1: `mem::zeroed()` on generic `SSAInfo<L>` -- This is a real safety issue, but the DX impact is indirect. Users cannot trigger it through normal builder APIs (which go through `finalize()` validation). The `finalize_unchecked` path is `pub(crate)`, so dialect authors never call it directly. Severity is correctly P0 for internal soundness but has low direct user-facing impact.

- **[severity-adjust: P2 -> P3]** Formalism P2 / Code-Quality P1 / Dialect-Author P2: DiGraph/UnGraph structural duplication -- Three reviewers flagged this independently, which confirms it is real. However, from a DX perspective dialect authors never interact with `DiGraphInfo` or `UnGraphInfo` internals; they use the builder API. The duplication is a maintainer concern, not a user concern. Adjust to P3 from a DX standpoint.

- **[agree]** Code-Quality P2: Rename `new()` to `build()` on builders -- This directly improves DX. Every Rust developer expects `new()` to return `Self`. The clippy suppression signals a convention violation that would confuse anyone reading builder call sites.

- **[agree]** Code-Quality P2: Missing `#[must_use]` on builders -- Directly user-facing. Silently dropping a builder is a time-wasting bug.

- **[false-positive]** Code-Quality P3: `language.rs` TODO comment -- A stale TODO does not affect users.

- **[agree]** Dialect-Author P2: `.port().port_name()` two-call pattern -- The reviewer correctly identifies that `.port_named("p0", MyType::F64)` would be more ergonomic. Graph port construction is a common operation for graph-dialect authors.

- **[severity-adjust: P3 -> P2]** Soundness P3: `port_name`/`capture_name` silently ignored in release -- From a DX perspective this is worse than P3. A dialect author who calls `port_name()` before `port()` loses the name silently with no feedback. The debug_assert-only guard means CI (release-mode tests) would not catch it. Upgrade to P2 from a user perspective.

### Low Priority Candidates

- Code-Quality P3: `unit_cmp` clippy suppression -- only affects the default `C = ()` constraint type. No user will encounter this.
- Dialect-Author P3: `pub(crate)` module visibility for digraph/ungraph -- informational only, docs are the real interface.

### Cross-Cutting Insights

- The builder API is the primary user-facing surface in U1. Three separate reviewers found issues (naming convention, missing `#[must_use]`, silent name loss), suggesting a single pass focused on builder DX polish would address all three.

---

## U2: Parser Runtime

### Reviewed Findings

- **[severity-adjust: P1 -> P2]** Formalism P1: `EmitContext` flat map lacks scoping -- Theoretically correct, but in practice the parser controls name generation and the text format uses `%0`, `%1`, etc. with monotonic numbering. User-authored IR text rarely has name collisions across scopes. The risk is real but the blast radius in current usage is low. Adjust to P2 from a DX standpoint; it becomes P1 if user-written IR text (not roundtripped) is a first-class use case.

- **[agree]** Formalism P2: `HasDialectEmitIR` is a public implementation detail -- Dialect authors should never see this trait. Marking it `#[doc(hidden)]` is a zero-cost DX improvement that removes API noise.

- **[agree]** Soundness P1: Duplicate SSA names silently shadow -- This is the highest-impact DX finding in U2. A user writing `%x = ...; %x = ...;` gets silent corruption. The error should be immediate and clear.

- **[severity-adjust: P2 -> P3]** Soundness P2: `expect` panics in pipeline parse -- Both soundness findings (P2 at :377 and :229) require either concurrent mutation or internal logic bugs. Normal users will never trigger these. Downgrade to P3 from DX perspective.

- **[agree]** Soundness P3: Duplicate block names silently shadow -- Same class as the SSA name issue but lower frequency (users rarely write block names manually). P3 is appropriate.

- **[agree]** Dialect-Author P1: No discoverable documentation for projection names -- This is the most impactful DX finding for dialect authors. Discovering that `{field:ports}` is valid but `{field:edges}` is not requires reading codegen source. A doc comment listing valid projections per field category would save significant time.

- **[severity-adjust: P2 -> P3]** Dialect-Author P2: Graph parser error messages lack context -- Error messages from chumsky are generic by nature. Improving labelling is nice-to-have but unlikely to block a dialect author who can already see the span.

- **[agree]** Code-Quality P2: `port_list`/`capture_list` duplication -- Confirms the dialect-author reviewer's finding. Minor.

- **[agree]** Code-Quality P1: `parse_text.rs` at 978 lines -- This is a file navigation issue, not a user-facing DX issue. Maintainer concern. P1 for code quality is fair but P2 from a user perspective.

### Low Priority Candidates

- Code-Quality P2: `#[allow(dead_code)]` on `TestDialect` -- test-only, zero user impact.
- Code-Quality P2: `Header.stage`/`Header.function` dead code -- internal parsing detail.
- Code-Quality P3: Missing `#[must_use]` on parser combinators -- Chumsky handles this upstream.

### Cross-Cutting Insights

- The silent-shadow problem (SSA names, block names) is the standout DX issue. Users who hand-write IR text get no feedback on duplicate names. This is the kind of bug that wastes hours of debugging time.
- Projection discoverability (knowing what `{field:X}` options exist) is a recurring friction point mentioned by both the dialect-author and formalism reviewers.

---

## U3: Derive Infrastructure

### Reviewed Findings

- **[false-positive]** Formalism P2: `Layout` 4-parameter associated type family -- The formalism reviewer correctly notes the complexity but concludes "no structural change required." From a DX perspective, derive macro authors interact with `Layout` once when setting up a new derive. The 4-type pattern is documented in AGENTS.md. Not a user problem.

- **[false-positive]** Formalism P2: `Template` returns `Vec<TokenStream>` -- The reviewer correctly concludes no change needed. Derive macro internals are not user-facing.

- **[agree]** Code-Quality P2: `misc.rs` grab-bag module -- Maintainer DX. The module name `misc` provides no navigational signal. Splitting into purpose-named files helps anyone modifying the derive infrastructure.

- **[agree]** Code-Quality P2: Manual `Clone` impls -- If the bounds already require `Clone`, removing 30 lines of manual impl reduces maintenance surface. Maintainer DX.

### Low Priority Candidates

- Code-Quality P2: `#[allow(clippy::large_enum_variant)]` on `Data<L>` and `FieldData<L>` -- constructed once during derive parsing, never hot-path. Zero user impact.
- Code-Quality P2: FieldInfo accessor pairs -- 15 small methods are more readable than a macro. Leave as-is.
- Code-Quality P3: `field_iter_set.rs` repetition -- inherent to codegen, not reducible without a meta-template.

### Cross-Cutting Insights

- U3 findings are almost entirely maintainer-facing. Dialect authors never touch `kirin-derive-toolkit` directly. No user-impacting DX issues found in this unit.

---

## U4: Parser/Printer Codegen

### Reviewed Findings

- **[agree]** Formalism P1: `FieldCategory` closed enum / expression problem -- The reviewer's conclusion is correct: new categories are rare (3 in the project lifetime). From a DX perspective, the exhaustive match guarantees that adding a new category produces compile errors at every site that needs updating, which is actually *good* DX for maintainers. The `#[non_exhaustive]` suggestion would make this worse (turning compile errors into runtime panics for downstream crates). Disagree with the mitigation, agree with the diagnosis.

- **[agree]** Formalism P2: Format string DSL lacks formal grammar -- This directly supports the U2 dialect-author finding about projection discoverability. An EBNF in the doc comment is the single highest-leverage documentation improvement for dialect authors. Upgrade to P1 from a DX perspective.

- **[severity-adjust: P1 -> P2]** Soundness P1: Parser/printer asymmetry for split Signature projections -- The reviewer flags a potential roundtrip issue. If this actually manifests, it is a compile-time error in generated code (the tuple types would not match), not a silent runtime bug. Add a roundtrip test as suggested, but the severity from a user perspective is P2 (broken compile, not silent corruption).

- **[agree]** Soundness P3: `expect` panics in codegen -- These fire during proc-macro expansion and produce unhelpful error messages. From a DX perspective, a derive macro panic is one of the worst user experiences. The P3 severity is appropriate because it requires a validation bug to trigger, but the fix (replacing `expect` with `syn::Error`) is low-effort and high-value.

- **[severity-adjust: P3 -> P2]** Soundness P3: `expect` for ir_path in graph body projections -- This one *is* user-triggerable: a misconfigured `#[kirin(crate = ...)]` on a graph-containing type. A dialect author outside the kirin workspace (where the default `::kirin::ir` does not resolve) would hit a proc-macro panic. Upgrade to P2.

### Low Priority Candidates

- Code-Quality P2: `ast_type()` match arm deduplication -- 10 lines saved, deeply internal.
- Code-Quality P2: `validation.rs` field count -- internal complexity, does not leak to users.
- Code-Quality P3: Unused `_ast_name`/`_type_params` parameters -- internal API cleanup.

### Cross-Cutting Insights

- The format string DSL is the dialect author's primary interface to the codegen system. Both the formalism reviewer (grammar specification) and the U2 dialect-author reviewer (projection discoverability) independently flagged documentation gaps. A single EBNF doc comment in `format.rs` addresses both findings.

---

## U5: Output & Dialects

### Reviewed Findings

- **[false-positive]** Formalism P2: `PrettyPrint` requires `L: PrettyPrint` recursively -- The reviewer correctly notes this is sound and the bound is standard. The dialect-author reviewer flags it as P1, but `#[derive(PrettyPrint)]` handles the bounds automatically. Only manual implementors see the bound, and they can follow kirin-function as a reference. The `PrettyPrintViaDisplay` escape hatch covers the simple case. Not a real user pain point.

- **[false-positive]** Formalism P2: `Lexical` vs `Lifted` isomorphism -- Two enums with 3 shared variants is not a DX problem. The enums are clearly named, serve different semantic purposes, and the `#[wraps]` derive eliminates boilerplate. Merging them would confuse the domain model.

- **[agree]** Code-Quality P1 (x3): print_digraph/ungraph/block body duplication -- The code-quality and dialect-author reviewers both flag this. From a DX perspective, this matters because dialect authors implementing custom printers might copy these patterns. Extracting shared helpers makes the reference implementation cleaner.

- **[agree]** Code-Quality P1: FunctionBody vs Lambda interpreter duplication -- Same justification: reference implementation quality. Dialect authors study kirin-function to learn the interpreter pattern.

- **[agree]** Dialect-Author P3: `Bind` has no interpreter support -- Runtime discovery of unsupported operations is poor DX. A `#[deprecated(note = "...")]` or a doc comment costs nothing and saves debugging time.

- **[severity-adjust: P1 -> P2]** Dialect-Author P1: `PrettyPrint` trait bound cascade -- Addressed above. The derive handles this; manual implementors have kirin-function as a reference. The real fix is better documentation, not a trait redesign. P2 is appropriate.

- **[agree]** Code-Quality P2: `ir_render.rs` at 604 lines -- Maintainer concern. Fair at P2.

- **[agree]** Code-Quality P2: Missing `#[must_use]` on `RenderBuilder` -- User-facing. A dropped `RenderBuilder` is a silent bug.

### Low Priority Candidates

- Code-Quality P3: `Call::interpret` at 70 lines -- the method is a linear chain of fallible lookups. Extracting a helper improves readability but does not change user experience.

### Cross-Cutting Insights

- kirin-function is the de facto reference dialect. Several DX improvements (interpreter dedup, `Bind` documentation, `#[must_use]` on `RenderBuilder`) compound to make it a better teaching tool. A single focused cleanup pass on kirin-function and kirin-prettyless would address most U5 findings.

---

## Global Cross-Cutting Themes

1. **Silent failures are the top DX risk.** Duplicate SSA names (U2), silently dropped port names (U1), and silently dropped builders (U1/U5) all share the same failure mode: the user does something wrong and gets no feedback. These should be prioritized as a class.

2. **The format string DSL is underdocumented.** Three independent reviewers (U2 dialect-author, U4 formalism, U4 code-quality) flagged discoverability of projection names and format syntax. A single EBNF grammar in a doc comment resolves all three findings.

3. **Reference implementation quality matters.** kirin-function is the primary teaching tool for dialect authors. Duplication in its printer and interpreter code (U5) sets a bad example for downstream authors to copy.

4. **Internal complexity does not equal user complexity.** Many findings in U3 (Layout trait, Template trait, FieldInfo accessors) and U4 (FieldCategory expression problem, validation struct field count) are deep internals that dialect authors never touch. These are correctly prioritized by their respective reviewers but should not compete for attention with user-facing issues.

5. **`expect` in proc-macro codegen is a DX antipattern.** Multiple soundness reviewers (U2, U4) flagged `expect` calls that produce unhelpful panics. In proc-macro context, a panic becomes an inscrutable "internal compiler error." Replacing `expect` with `syn::Error` or `map_err` across the codegen crates is a systematic improvement.
