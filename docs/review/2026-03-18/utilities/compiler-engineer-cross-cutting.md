# Utilities — Compiler Engineer Cross-Cutting Review

**Crates:** kirin-lexer, kirin-interval (~2445 lines)

---

## Findings

### U-CC-1. `kirin-lexer` has minimal deps and clean feature gating
**Severity:** Positive | **Confidence:** High
**Files:** `crates/kirin-lexer/Cargo.toml:7-12`

Only `logos` as a mandatory dependency; `proc-macro2` and `quote` gated behind a `quote` feature. This is the leanest crate in the workspace. No issues.

### U-CC-2. `kirin-lexer` `ToTokens` impl is a 100-line manual match
**Severity:** P3 | **Confidence:** Medium
**Files:** `crates/kirin-lexer/src/lib.rs:144-244`

The `ToTokens` impl for `Token` manually maps each variant. Adding a new token requires updating three places: the enum definition, `Display`, and `ToTokens`. A macro could unify the token definition, but this is low-priority since the token set is stable.

### U-CC-3. `kirin-interval` lattice implementation scales correctly
**Severity:** Positive | **Confidence:** High
**Files:** `crates/kirin-interval/src/interval/lattice_impl.rs`, `crates/kirin-interval/src/interval/arithmetic.rs`

`Lattice::join`, `meet`, and `is_subseteq` are all O(1) operations on `Bound` pairs. Arithmetic operations (`interval_add`, `interval_sub`, `interval_mul`) are O(1). Multiplication uses the 4-product corner approach (`crates/kirin-interval/src/interval/arithmetic.rs:27-34`), which is standard and correct. Widening and narrowing (`crates/kirin-interval/src/interval/interpreter_impl.rs:29-62`) are O(1). This scales to any number of abstract values in the analysis.

### U-CC-4. `kirin-interval` Div and Rem return Top unconditionally
**Severity:** P2 | **Confidence:** High
**Files:** `crates/kirin-interval/src/interval/lattice_impl.rs:83-96`

`Interval::div` and `Interval::rem` both return `Interval::top()`, ignoring the operands entirely. While safe (overapproximation), this means any program with division loses all interval precision through that operation. Standard interval analysis computes tighter bounds for division when the divisor does not span zero (e.g., `[a,b] / [c,d]` where `c > 0` gives `[a/d, b/c]`). Similarly, remainder can be bounded by the divisor's absolute value.

**Recommendation:** Implement tighter bounds for non-zero-spanning divisors. The existing `CheckedDiv` and `CheckedRem` impls (`crates/kirin-interval/src/interval/arith_impl.rs:5-15`) also return `Some(Interval::top())`, so both paths need updating.

### U-CC-5. `kirin-interval` only supports `i64`-based intervals
**Severity:** P3 | **Confidence:** Medium
**Files:** `crates/kirin-interval/src/interval/bound.rs:3`, `crates/kirin-interval/src/interval/domain.rs:11`

`Bound::Finite(i64)` and `Interval::new(lo: i64, hi: i64)` hardcode the integer type. For `ArithValue` conversion (`crates/kirin-interval/src/interval/arith_impl.rs:17-29`), values like `f32`/`f64` map to `top()`. This is a design limitation rather than a bug -- supporting floating-point intervals would require separate domain types. Acceptable for the current scope but worth documenting.

### U-CC-6. `kirin-interval` feature flags are well-structured
**Severity:** Positive | **Confidence:** High
**Files:** `crates/kirin-interval/Cargo.toml:16-19`

Three independent features (`interpreter`, `arith`, `cmp`) each gating exactly one optional dependency. The core interval domain compiles with only `kirin-ir`. This means adding new dialect integration (e.g., `bitwise`) only adds a new feature without affecting existing users.

---

**Summary:** Both utility crates are well-structured. The main actionable finding is that `kirin-interval`'s division/remainder returning `Top` unconditionally (U-CC-4) degrades analysis precision significantly for programs with division. The lexer is minimal and stable. The interval domain scales correctly in computational complexity.
