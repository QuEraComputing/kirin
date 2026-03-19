# Dialects -- Final Review Report

**Crates:** kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-cmp, kirin-function (~3295 lines)
**Input reports:** PL Theorist (formalism), Implementer (code quality), Physicist (ergonomics), Compiler Engineer (cross-cutting)

---

## High Priority (P0-P1)

### D-P1-1. Dialect crates depend on top-level `kirin`, pulling full parser+printer unconditionally
**Source:** Compiler Engineer D-CC-1 | **Confidence:** High
**Files:** All 7 dialect `Cargo.toml` files (e.g., `crates/kirin-arith/Cargo.toml:7`)

All dialect crates have `kirin.workspace = true` as a mandatory dependency. The top-level `kirin` unconditionally depends on `kirin-chumsky` (with `derive`), `kirin-prettyless` (with `derive`), `chumsky`, and `kirin-lexer`. A user who only wants IR types + interpreter for a dialect must compile the entire parser and printer stack. At scale (50+ dialects), this creates unnecessary compilation fan-out.

**Recommendation:** Dialect crates should depend on `kirin-ir` directly. Feature-gate `HasParser` and `PrettyPrint` derives behind `parser`/`pretty` features, mirroring the existing `interpret` feature pattern. The `kirin` umbrella crate remains for end-user convenience.

**Reinforces Phase 1:** P1-2 (kirin-chumsky hard-depends on kirin-prettyless) and P1-3 (bat default feature). These are all part of the same dependency hygiene theme.

### D-P1-2. Inner dialect enums lack `#[derive(Interpretable)]` support
**Source:** Physicist D2, Implementer D3 | **Confidence:** High
**Files:** `crates/kirin-function/src/interpret_impl.rs:97-116` (Lexical), `:282-301` (Lifted), `crates/kirin-scf/src/interpret_impl.rs:229-247` (StructuredControlFlow)

Three inner dialect enums (`Lexical`, `Lifted`, `StructuredControlFlow`) manually delegate `interpret()` to inner variants -- exactly what `#[derive(Interpretable)]` already does for top-level language enums. This is ~69 lines of pure delegation boilerplate.

**Reinforces Phase 1:** P1-8 directly. Two reviewers independently confirmed the same 3 enums and line count.

---

## Medium Priority (P2)

### D-P2-1. Binary-op interpret boilerplate across Arith/Bitwise/Cmp
**Source:** Implementer D1 | **Confidence:** High
**Files:** `crates/kirin-arith/src/interpret_impl.rs:37-91`, `crates/kirin-bitwise/src/interpret_impl.rs:26-73`, `crates/kirin-cmp/src/interpret_impl.rs:235-284`

All three crates repeat the same read-lhs, read-rhs, apply-op, write-result, return-Continue pattern. A helper like `interp.binary_op(lhs, rhs, result, |a, b| a + b)` would eliminate ~100 lines. Note: this is a separate concern from D-P1-2 -- even with `#[derive(Interpretable)]` on inner enums, the individual variant interpret bodies still have this boilerplate.

**Related to Phase 1:** P2-H (Interpretable trait bound simplification). The boilerplate is partly structural (L-on-method where clauses) and partly operational (the binary-op pattern).

### D-P2-2. Interpreter where-clause boilerplate on every manual `Interpretable` impl
**Source:** Physicist D1 | **Confidence:** High
**Files:** All 7 dialect `interpret_impl.rs` files, ~14 manual impls

Every `Interpretable` impl repeats the same 3-line method-level where clause:
```rust
I::StageInfo: HasStageInfo<L>,
I::Error: From<InterpreterError>,
L: Interpretable<'ir, I> + 'ir,
```
This is the inherent cost of L-on-method (which is intentional and correct). Impact will decrease as D-P1-2 is addressed (derive generates these automatically). For remaining manual impls (e.g., `Call`, `For`, `If`), this is unavoidable unless the trait bounds are simplified.

**Reinforces Phase 1:** P2-H directly.

### D-P2-3. FunctionBody/Lambda SSACFGRegion + Interpretable duplication
**Source:** Implementer D3 | **Confidence:** High
**Files:** `crates/kirin-function/src/interpret_impl.rs:9-43` vs `:45-79`

`FunctionBody` and `Lambda` have identical `SSACFGRegion::entry_block` and `Interpretable::interpret` implementations (get first block from region, return Jump). ~35 lines of exact duplication. A blanket impl over a `HasRegionBody` trait or a shared free function would eliminate this.

### D-P2-4. `CompareValue` returns `Self` instead of a boolean domain
**Source:** PL Theorist D1 | **Confidence:** Medium
**Files:** `crates/kirin-cmp/src/interpret_impl.rs:10-17`

`cmp_eq(&self, other: &Self) -> Self` conflates the comparison operand domain with the boolean result domain. The `Interval` implementation works around this by returning `Interval::new(0, 1)`. For concrete execution this is fine; for abstract interpretation it limits precision. A separate `BoolDomain` associated type would be cleaner but adds complexity.

**Assessment:** Valid concern for abstract interpretation precision. Medium priority because it only affects `kirin-interval` users today.

---

## Low Priority (P3)

### D-P3-1. `_ => unreachable!()` in `#[non_exhaustive]` enum match arms
**Source:** Implementer D2 | **Confidence:** High
**Files:** `kirin-arith/src/interpret_impl.rs:91`, `kirin-bitwise/src/interpret_impl.rs:74`, `kirin-cmp/src/interpret_impl.rs:284`, `kirin-cf/src/interpret_impl.rs:61`

Wildcard catch-all handles the `__Phantom` variant but would silently absorb new variants. Matching `Self::__Phantom(..) => unreachable!()` explicitly would restore exhaustiveness checking.

### D-P3-2. `ForLoopValue::loop_condition` undocumented `None` semantics for abstract interpretation
**Source:** PL Theorist D2 | **Confidence:** Medium
**File:** `crates/kirin-scf/src/interpret_impl.rs:199`

`None` from `loop_condition` silently terminates the loop (the `while` condition fails). For abstract interpreters, this means the "unknown" case is treated as "false" -- the loop body is never explored when the condition is indeterminate. Correct for concrete execution, but should be documented.

### D-P3-3. `CheckedDiv` for floats -- naming mismatch
**Source:** PL Theorist D4 | **Confidence:** Medium
**File:** `crates/kirin-arith/src/checked_ops.rs:33-36`

Float `CheckedDiv` returns `Some(self / rhs)` even for division by zero (producing infinity/NaN). The "checked" name implies fallibility. Document that "checked" means "non-panicking" rather than "error-detecting."

### D-P3-4. `Lexical` vs `Lifted` lack a shared trait witness for the closure conversion relationship
**Source:** PL Theorist D3 | **Confidence:** Low
**File:** `crates/kirin-function/src/lib.rs:40-55`

Both share `FunctionBody`, `Call`, `Return` but differ in `Lambda` vs `Bind`. The transformation relationship is informal. Only matters if/when closure conversion passes are added.

### D-P3-5. `ArithValue` manual PartialEq/Hash/Display (~100 lines)
**Source:** Physicist D3 | **Confidence:** Medium
**File:** `crates/kirin-arith/src/types/arith_value.rs:51-157`

Mechanical match arms for 3 trait impls. Special casing for f32/f64 `to_bits` makes a simple derive non-trivial. One-time cost; low priority.

### D-P3-6. `Interval` fields are `pub`
**Source:** Implementer D4 | **Confidence:** Medium
**File:** `crates/kirin-interval/src/interval/domain.rs:5-8`

Public `lo`/`hi` fields allow constructing invalid intervals bypassing the `new()` normalizer. Related to Phase 1 P3-3 (Signature fields pub).

---

## Strengths

1. **Highly consistent dialect structure.** All 7 dialects follow the same pattern: `lib.rs` (enum + derives), `interpret_impl.rs` (behind `#[cfg(feature = "interpret")]`), `tests.rs` (behind `#[cfg(test)]`). Learning one teaches all. (Physicist D4, Implementer summary)

2. **Zero cross-dependencies between dialect crates.** `kirin-bitwise` does not depend on `kirin-arith`, etc. Supports parallel compilation and clean layering. (Compiler Engineer D-CC-3)

3. **Format string DSL eliminates parser/printer code.** Dialect authors write `#[chumsky(format = "...")]` and get both parsing and pretty-printing for free. Zero manual parser/printer code in any dialect crate. (All reviewers)

4. **Strong MLIR alignment.** The cf/scf/function decomposition follows MLIR conventions (Block vs Region for SingleBlock ops, proper Block for SCF bodies). (PL Theorist summary)

5. **Clean `#[allow]` audit.** Only 1 `#[allow(clippy::derivable_impls)]` across all 7 crates, and it is justified (`ArithType::Default` returns `I64`, not the first variant). (Implementer)

6. **Feature gating for interpreter.** The `interpret` feature pattern is consistently applied across all 7 dialects. (Implementer summary)

---

## Filtered Findings

| Finding | Source | Reason for Filtering |
|---------|--------|---------------------|
| PhantomData on generic dialect types | Multiple | Intentional per AGENTS.md design conventions |
| Builder panics in dialect constructors | Multiple | Intentional -- programmer errors, panics correct per design context |
| L-on-method is complex | Physicist D1 (implicit) | Intentional design -- breaks E0275 cycle |
| Cargo.toml structure repetition across dialects | Compiler Engineer D-CC-2 | Not actionable now; 7 crates is manageable |
| Monomorphization at 50 dialects | Compiler Engineer D-CC-4 | Speculative; O(N) match arms is acceptable; monitor only |
| `Lexical`/`Lifted` lack shared trait | PL Theorist D3 | Speculative; only matters when closure conversion passes are added |

---

## Suggested Follow-Up Actions

### Quick Wins (< 30 min each)
1. Replace `_ => unreachable!()` with `Self::__Phantom(..) => unreachable!()` in all 4 dialect interpret impls (D-P3-1)
2. Document `ForLoopValue::loop_condition` `None` semantics for abstract interpretation (D-P3-2)
3. Document `CheckedDiv` "non-panicking" semantics for floats (D-P3-3)

### Moderate Effort (1-3 hours each)
4. Extract binary-op helper for Arith/Bitwise/Cmp interpret impls (D-P2-1)
5. Extract shared `SSACFGRegion` + `Interpretable` impl for FunctionBody/Lambda (D-P2-3)
6. Make `Interval` fields private with accessors (D-P3-6, alongside Phase 1 P3-3)

### Design Work (half-day+)
7. **[Highest impact]** Decouple dialect crates from top-level `kirin` -- depend on `kirin-ir` directly, feature-gate parser/printer derives (D-P1-1). This is a structural change affecting all 7 crates and requires coordinating with Phase 1 P1-2 and P1-3.
8. Extend `#[derive(Interpretable)]` for inner dialect enums with `#[wraps]` (D-P1-2, reinforcing P1-8)
9. Evaluate `CompareValue` boolean domain separation for abstract interpretation (D-P2-4)

### Phase 1 Reinforcement Summary
| This Report | Phase 1 | Theme |
|-------------|---------|-------|
| D-P1-1 | P1-2, P1-3 | Dependency hygiene |
| D-P1-2 | P1-8 | Inner dialect derive |
| D-P2-2 | P2-H | Where-clause boilerplate |
| D-P3-6 | P3-3 | Public fields on domain types |
