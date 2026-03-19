# Utilities -- PL Theorist Formalism Review

## Findings

### U1. Interval `saturating_add` for NegInf + PosInf resolves to NegInf -- conservative but asymmetric (P2, confirmed)

`kirin-interval/src/interval/bound.rs:42`: `Bound::saturating_add(NegInf, PosInf)` returns `NegInf`. This is mathematically undefined (indeterminate form), and the choice of `NegInf` introduces asymmetry -- `PosInf + NegInf` also gives `NegInf` (same arm). The standard approach in interval arithmetic (IEEE 1788) would be to return the entire real line (top), not a single bound. In Kirin's context, this means `interval_add` on intervals that span both infinities may produce an unsound lower bound. For example, `[-inf, 5] + [-3, +inf]` gives `lo = NegInf + (-3) = NegInf` and `hi = 5 + PosInf = PosInf`, which happens to be correct, but the underlying bound arithmetic is fragile for edge cases.

Similarly, `saturating_sub(NegInf, NegInf)` at line 51 returns `NegInf` and `saturating_sub(PosInf, PosInf)` returns `NegInf`. The `PosInf - PosInf = NegInf` case is particularly concerning: it biases the result downward for an indeterminate form. This could produce unsound *under-approximations* if an interval's bounds hit these cases.

**File:** `crates/kirin-interval/src/interval/bound.rs:40-56`

### U2. Interval division and remainder return `top()` unconditionally (P2, confirmed)

`kirin-interval/src/interval/lattice_impl.rs:82-96`: `Div` and `Rem` for `Interval` return `Interval::top()` regardless of input. This is sound (an over-approximation), but loses all precision. Standard interval division (when the divisor interval does not contain zero) has a well-known formula: `[a,b] / [c,d] = [min(a/c, a/d, b/c, b/d), max(...)]`. The top-returning implementation means any analysis involving division immediately loses all information about the result.

**Alternative formalisms**: (1) Full interval division with zero-exclusion check -- most precise, moderate complexity. (2) Case-split on divisor sign (positive/negative/contains-zero) -- good precision/complexity tradeoff. (3) Current top() -- simplest, sound, but imprecise. Given the existing infrastructure for multiplication (4-corner formula at `arithmetic.rs:23-36`), implementing proper division would follow the same pattern.

**File:** `crates/kirin-interval/src/interval/lattice_impl.rs:82-96`

### U3. Widening operator matches Cousot-Cousot standard form (positive, confirmed)

`kirin-interval/src/interval/interpreter_impl.rs:29-47`: The widening operator follows the standard CC77 definition -- if the new bound exceeds the current bound, jump to infinity. Narrowing at lines 49-62 also follows the standard pattern (replace infinite bounds with finite ones from the new iteration). Both are correctly implemented.

**File:** `crates/kirin-interval/src/interval/interpreter_impl.rs:29-62`

### U4. Lexer negative integer ambiguity (P3, confirmed)

`kirin-lexer/src/lib.rs:35`: The regex `-?[0-9]+` for `Int` means `-42` is lexed as a single negative integer token. This creates a lexical ambiguity: in `%x -42`, is `-42` a negative literal or subtraction of `42` from `%x`? The lexer resolves this by longest-match (greedy), always producing `Int("-42")`. This is a standard design choice (C/Java do it differently with unary minus), but it means the parser must handle the case where subtraction of a literal looks different from subtraction of a variable. The test at line 921-928 documents the `-> -1` disambiguation.

**File:** `crates/kirin-lexer/src/lib.rs:35`

### U5. `Bound::negate` for `i64::MIN` returns `PosInf` (P3, confirmed)

`kirin-interval/src/interval/bound.rs:85-88`: `Finite(i64::MIN).negate()` returns `PosInf` because `checked_neg` fails for `i64::MIN`. This is sound (an over-approximation since `|i64::MIN| > i64::MAX`), but it means negating the interval `[i64::MIN, i64::MIN]` produces `[PosInf, PosInf]` rather than bottom or a more precise representation. In practice this is unlikely to cause issues since `i64::MIN` is a boundary value.

**File:** `crates/kirin-interval/src/interval/bound.rs:85-88`

## Summary

The interval domain is well-implemented for its scope, with correct widening/narrowing and lattice structure. The two P2 findings are U1 (indeterminate-form bound arithmetic biases results) and U2 (division/remainder loses all precision). U1 is the more concerning issue as it could produce unsound under-approximations in edge cases involving infinite bounds. The lexer is clean and well-tested.
