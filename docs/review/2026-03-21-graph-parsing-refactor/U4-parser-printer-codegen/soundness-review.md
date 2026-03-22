# U4: Parser/Printer Codegen -- Soundness Review

## Invariant Inventory

| Invariant | Location | Enforcement |
|-----------|----------|-------------|
| Format fields match struct fields | validation.rs:95-123 | Compile-time (proc-macro error) |
| Body projection completeness (ports+captures+body for graphs) | validation.rs:128-175 | Compile-time (proc-macro error) |
| ResultValue fields must not use :name in format | validation.rs:86-92 | Compile-time (proc-macro error) |
| SSA fields must have name occurrence | validation.rs:112-123 | Compile-time (proc-macro error) |
| No duplicate default occurrences | validation.rs:332-343 | Compile-time (proc-macro error) |
| Body projections valid for field category | validation.rs:279-321 | Compile-time (proc-macro error) |
| Occurrence count matches field count in format | codegen/parser/chain.rs:52 | Runtime (expect during codegen) |
| Format string parses successfully | codegen/pretty_print/statement.rs:192 | Runtime (expect during codegen) |
| ir_path present for graph body projections | field_kind.rs:297,328 | Runtime (expect during codegen) |

## Findings

### [P1] [likely] Parser/printer asymmetry for Signature field with `:inputs`/`:return` projections -- field_kind.rs:210-224 vs 364-373

**Invariant:** Parser and printer should produce symmetric output for roundtrip correctness.

**Enforcement:** Not validated. The parser for `{sig:inputs}` produces `Vec<T>` and `{sig:return}` produces `T`. The EmitIR codegen clones the Signature directly (field_emit.rs:72). However, the parser chain for split projections (`{sig:inputs}` ... `{sig:return}`) produces *two separate* parsed values that must be reassembled into a `Signature`. The chain builder produces a tuple of `(Vec<T>, T)` but the AST constructor must stitch them back into `Signature::new(inputs, ret, ())`.

**Attack:** A dialect struct with `#[kirin(format = "({sig:inputs}) -> {sig:return}")]` where `sig: Signature<T>`. If the chain builder does not correctly reassemble the two parsed halves into a single Signature in the AST constructor, the emitted IR would have a broken signature.

**Consequence:** Silent signature corruption or compile error in generated code.

**Reachability:** Normal use -- any dialect using split signature projections.

**Suggested mitigation:** Add a roundtrip test specifically for split signature projections to verify end-to-end correctness.

### [P3] [confirmed] `expect` in codegen panics on internal inconsistency -- chain.rs:52,538,540; statement.rs:190,192

**Invariant:** Occurrence sequence and format elements must match after validation.

**Enforcement:** Runtime (expect during proc-macro expansion). These are post-validation invariants -- validation should catch mismatches before codegen runs.

**Attack:** If validation has a bug that lets through a format string with mismatched field/occurrence counts, the `expect("occurrence sequence mismatch")` at chain.rs:52 will panic during proc-macro expansion, producing an `internal compiler error`-style message rather than a helpful diagnostic.

**Consequence:** Unhelpful ICE-style panic during compilation instead of a proper `syn::Error`.

**Reachability:** Adversarial (requires validation bug).

**Suggested mitigation:** Replace `expect` with `ok_or_else` returning `syn::Error` for defense-in-depth.

### [P3] [confirmed] `expect` for ir_path in graph body print expressions -- field_kind.rs:297,328

**Invariant:** `ir_path` must be `Some` when generating body projection print code for DiGraph/UnGraph.

**Enforcement:** Runtime (expect during codegen). The `ir_path` comes from the `#[kirin(crate = ...)]` attribute. If a user omits it and the default resolution fails, the `expect` fires during proc-macro expansion.

**Attack:** Write a derive macro usage without `#[kirin(crate = ...)]` in a crate where the default `::kirin::ir` path does not resolve, on a type with DiGraph/UnGraph body projections.

**Consequence:** Panic during proc-macro expansion with unhelpful message.

**Reachability:** Normal use (misconfigured crate path).

**Suggested mitigation:** Validate ir_path presence during the validation phase for types with graph body projections.

## Strengths

- Comprehensive compile-time validation catches the majority of format string errors before codegen runs.
- Body projection completeness checking (ports+captures+body required for graphs) prevents roundtrip-breaking partial projections.
- ResultValue name handling is correctly migrated to generic parsing, with legacy usage explicitly rejected.
- Validation distinguishes field categories precisely, preventing `:name`/`:type` on non-SSA fields and invalid body projections on wrong field types.
- The visitor pattern (`FormatVisitor`) separates validation concerns from codegen, making the validation logic testable independently.
