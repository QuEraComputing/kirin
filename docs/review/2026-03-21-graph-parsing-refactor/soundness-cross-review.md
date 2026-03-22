# Soundness -- Cross-Review

## U1: Core IR

### Reviewed Findings

- **[agree, P0]** U1-formalism `mem::zeroed()` on generic `SSAInfo<L>` in `finalize_unchecked` -- This is a genuine soundness bug, and I would elevate the concern beyond what the formalism reviewer stated. The issue affects *both* `finalize()` (line 222) and `finalize_unchecked()` (lines 262, 272). The `map_live` tombstone closure `|_info| unsafe { std::mem::zeroed() }` produces a zeroed `SSAInfo<L>` that is stored in `Item.data: SSAInfo<L>` (not `Option`). When the `Arena` is dropped, `Vec<Item<SSAInfo<L>>>` drops every item including tombstones. `SSAInfo<L>` contains `ty: L::Type` and `uses: SmallVec<[Use; 2]>` -- zeroed `SmallVec` has an invalid discriminant/pointer, and any heap-allocating `L::Type` (e.g., `String`, `Vec<T>`) has a null pointer. Both trigger UB on drop. The arena already has `try_map_live_option` (data.rs:109) that uses `Option<U>` with `None` tombstones -- switching to that API is the minimal fix, though it requires `StageInfo.ssas` to become `Arena<SSAValue, Option<SSAInfo<L>>>`. The safety comment "deleted items are tombstoned and never dereferenced" is irrelevant -- the UB occurs in `Drop`, not dereference.

- **[agree, severity-adjust P2->P3]** U1-formalism `DiGraphInfo`/`UnGraphInfo` structural duplication -- Correct observation. No soundness implication. The duplication is a maintenance burden but both implementations are independently correct.

- **[agree, severity-adjust P1->P2]** U1-code-quality DiGraphBuilder vs UnGraphBuilder duplication -- Same as above, code quality not soundness. The parallel logic is identical in behavior so there is no divergence risk yet, making P2 more appropriate.

- **[agree]** U1-code-quality `#[allow(clippy::wrong_self_convention)]` on builders -- Correct. No soundness implication. Renaming `new()` to `build()` is a clean fix.

- **[agree]** U1-ergonomics port placeholder creation lacks a dedicated builder method -- Correct. Not a soundness issue but a real usability gap that could lead to incorrect placeholder construction.

- **[agree]** U1-code-quality missing `#[must_use]` on builder types -- Correct. Silently dropping a builder is not UB but can cause logic errors (allocated arena slots with no finalized node). Low soundness impact because the dangling builder slots are cleaned up by validation in `finalize()`.

### Low Priority Candidates

- U1-code-quality `language.rs` TODO comment -- Zero soundness impact, pure housekeeping.
- U1-code-quality `#[allow(clippy::unit_cmp)]` on signature semantics -- Zero soundness impact; the `()` comparison is correct, just redundant.
- U1-dialect-author `pub(crate) mod` with `pub use` re-exports -- Documentation discoverability, not soundness.

### Cross-Cutting Insights

- The `mem::zeroed()` finding in U1-formalism interacts with U2's forward-reference mechanism. `create_forward_ref` (emit_ir.rs:118) creates SSAs with `ty: None` and `BuilderSSAKind::Unresolved`. If such SSAs are later deleted (e.g., replaced during graph resolution), they become tombstoned slots that hit the `mem::zeroed()` path in `finalize_unchecked`. This means the forward-reference parsing path (U2) is a *trigger* for the U1 soundness bug -- not just the builder API directly.

---

## U2: Parser Runtime

### Reviewed Findings

- **[agree, severity-adjust P1->P0]** U2-formalism `EmitContext` flat map scoping -- I would elevate this. The formalism reviewer correctly identified the lack of scope push/pop, but underestimated the severity. In `emit_block` (blocks.rs:158-160), block arguments are registered directly into the flat `ssa_names` map with `ctx.register_ssa(name, ssa)`. When a Region emits multiple blocks (blocks.rs:268), each block's arguments overwrite prior entries for the same name. For nested constructs (e.g., `scf.if` inside a function body where both define `%x`), the outer `%x` is permanently lost. This is not just fragile -- it produces *silently wrong IR* where the outer reference resolves to the inner SSA. The two-pass Region emission (blocks.rs:246-269) does not introduce any scoping either. In MLIR, this is a hard requirement (OpAsmParser maintains a scope stack). For single-block operations this is masked because there's no nesting, but for any dialect with Region fields containing multiple blocks with overlapping names, this is a correctness bug.

- **[agree]** U2-formalism `HasDialectEmitIR` witness trait visibility -- Correct. Making it `#[doc(hidden)]` or `pub(crate)` would prevent accidental dependence. No soundness issue but a semver hazard.

- **[agree]** U2-code-quality `parse_text.rs` at 978 lines -- Correct, no soundness impact. Pure maintainability.

- **[agree]** U2-code-quality `port_list()` vs `capture_list()` duplication -- Correct, trivially fixable. No soundness impact.

- **[agree]** U2-ergonomics three `ParseEmit` paths decision paralysis -- Correct. No soundness impact, but choosing the wrong path (e.g., `SimpleParseEmit` on a dialect with Block fields) would be caught at compile time, so the risk is limited to confusion, not incorrectness.

- **[severity-adjust P1->P2]** U2-ergonomics `EmitContext` forward-reference mode invisible to users -- Correct that the API is subtle, but `set_relaxed_dominance` is called internally by the graph emit code (graphs.rs:189, 289), not by users. Dialect authors using `#[derive(HasParser)]` never call it directly. The concern is valid for manual `EmitIR` implementors working with custom graph types, but that is a narrow audience. P2 is more appropriate.

- **[false-positive]** U2-ergonomics `parse_ast` returns `Vec<ParseError>` not `ChumskyError` -- This is by design. `parse_ast` is the AST-only API (no emission), so it returns parse-only errors. `parse_statement` does parse+emit, so it returns `ChumskyError` which wraps both. The types correctly reflect the phase boundary.

- **[agree]** U2-dialect-author no discoverable documentation for projection names -- Correct. A misspelled projection name surfaces at codegen time (the derive macro rejects it), not at runtime, so this is a DX issue, not a soundness issue.

### Low Priority Candidates

- U2-code-quality `#[allow(dead_code)]` on `TestDialect` -- Test-only, zero impact.
- U2-code-quality `#[allow(dead_code)]` on `Header.stage/function` -- Grammar documentation fields, no impact.
- U2-code-quality no `#[must_use]` on parser combinators -- Chumsky's own lazy evaluation model handles this; dropping a parser is not UB.

### Cross-Cutting Insights

- The `EmitContext` flat-map scoping issue (U2-formalism P1) has a direct interaction with U1's forward-reference system. When `set_relaxed_dominance(true)` is active (graphs.rs:189), `resolve_ssa` creates placeholder SSAs for undefined names. If a name collision occurs (inner block argument shadows outer), the flat map silently resolves to the wrong SSA. Under relaxed dominance, even the *wrong* SSA resolves successfully (no error), so the bug is completely silent. This is the worst combination: silent wrong IR with no diagnostic.

- The `ChumskyError` naming concern (U2-ergonomics P2) is actually more significant from a soundness perspective than the reviewer noted. `ChumskyError::Emit(EmitError)` errors include `UndefinedSSA` which, due to the flat-map scoping issue, may *not* fire when it should (shadowed names resolve to the wrong SSA instead of erroring). So the error type's semantics are subtly weaker than documented.

---

## U4: Parser/Printer Codegen

### Reviewed Findings

- **[agree, severity-adjust P1->P2]** U4-formalism `FieldCategory` closed enum expression problem -- Correct analysis, but the reviewer's own conclusion ("the closed enum is acceptable") contradicts the P1 severity. New field categories are rare (3 in the project's lifetime) and every addition is deliberate. Exhaustive match is a *feature* here -- it ensures all derive crates handle new categories. Adding `#[non_exhaustive]` would make it worse (silent `_ =>` fallthrough instead of compile errors). P2 is appropriate.

- **[agree]** U4-formalism format string DSL lacks formal grammar -- Correct. No soundness implication because the DSL is parsed at compile time by Chumsky combinators with deterministic behavior. Ambiguity analysis is nice-to-have for documentation but the parser-is-the-grammar approach is sound in practice for a non-adversarial DSL.

- **[agree]** U4-code-quality `chain.rs` at 615 lines -- Maintainability concern. No soundness impact.

- **[agree]** U4-code-quality `validation.rs` high field count -- Correct. No soundness impact; the validation logic is correct.

- **[agree]** U4-ergonomics format string not documented outside source -- DX issue, no soundness impact. Format errors are caught at compile time.

- **[agree]** U4-ergonomics body projection completeness checking is strict -- Correct and *desirable* for roundtrip soundness. The strictness ensures parse-print roundtrip correctness. A `#[kirin(no_captures)]` escape hatch would need careful validation to avoid breaking roundtrip invariants.

- **[false-positive]** U4-ergonomics `$keyword` vs `{.keyword}` migration error -- The reviewer suggests a "Did you mean `$add`?" hint for unrecognized identifiers, but unrecognized identifiers in format strings are treated as literal tokens (which is correct behavior for format strings like `"fn {name}"`). Adding a heuristic guess would be fragile.

- **[agree]** U4-ergonomics `RenderDispatch` undocumented -- Correct. No soundness impact; the derive is simple dispatch generation.

- **[agree]** U4-code-quality unused `_ast_name`/`_type_params` parameters -- Correct. Dead parameters are not a soundness issue.

### Low Priority Candidates

- U4-code-quality `field_kind.rs` `ast_type()` match arm duplication -- Trivial code dedup, no impact.
- U4-code-quality parallel category dispatching in pretty_print vs parser -- Structural observation, not actionable without significant redesign.

### Cross-Cutting Insights

- The format string validation system (U4-formalism strength: `ValidationVisitor` checks roundtrip completeness) is the key defense against the `EmitContext` scoping issue in U2. If validation ensures all fields appear in the format string, then the parse-print roundtrip is *syntactically* complete even if the *semantic* SSA resolution is wrong. This means the roundtrip tests pass but with wrong SSA bindings -- making the U2 scoping bug harder to detect through testing.

- The `FieldCategory` closed enum (U4-formalism P1) is related to the U1 duplication findings. `DiGraph` and `UnGraph` being separate `FieldCategory` variants is what forces the duplication in U4's `ast_type()`, `parser_expr()`, and `print_expr()`. If U1 unifies `DiGraphInfo`/`UnGraphInfo` into `GraphInfo<D>`, the corresponding unification in U4 (a single `Graph` category) would reduce the expression-problem cost.

---

## Summary: Top Soundness Priorities

| Priority | Unit | Finding | Impact |
|----------|------|---------|--------|
| P0 | U1 | `mem::zeroed()` on `SSAInfo<L>` in arena tombstones | UB on drop for any heap-allocating `L::Type`. Triggered by normal graph parsing flow (forward refs -> deleted SSAs -> zeroed tombstones). |
| P0 | U2 | `EmitContext` flat-map scoping | Silent wrong IR when block argument names collide across nested blocks/regions. Interacts with relaxed dominance to suppress error diagnostics. |
| P2 | U2 | `HasDialectEmitIR` is `pub` but should be hidden | Semver hazard, no UB. |
| P2 | U1 | DiGraph/UnGraph duplication | Maintenance burden, no correctness risk currently. |
