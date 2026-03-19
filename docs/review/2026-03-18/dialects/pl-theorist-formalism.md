# Dialects -- PL Theorist Formalism Review

## Findings

### D1. `CompareValue` returns `Self` instead of a boolean domain (P2, confirmed)

`CompareValue` in `kirin-cmp/src/interpret_impl.rs:10-17` defines `cmp_eq(&self, other: &Self) -> Self`. The return type is the *same* as the operand type, encoding booleans as integers (1/0). This conflates two distinct domains: the comparison operand domain and the boolean result domain. In standard PL literature (e.g., Cousot & Cousot's abstract domain framework), comparison operations produce elements of a *boolean abstract domain*, not the operand domain. The `Interval` implementation at `kirin-interval/src/interval/cmp_impl.rs:4-85` works around this by returning `Interval::new(0, 1)` -- a valid but imprecise encoding where the boolean domain collapses into the integer interval domain.

**Alternative formalisms**: (1) Separate `BoolDomain` associated type on `CompareValue` -- clean separation but adds a type parameter. (2) Return `Option<bool>` like `BranchCondition::is_truthy` -- simpler but loses abstract domain structure. (3) Current approach -- pragmatic, zero extra types, but semantically imprecise for abstract interpretation.

**File:** `crates/kirin-cmp/src/interpret_impl.rs:10-17`

### D2. `ForLoopValue::loop_condition` returns `Option<bool>` without documented lattice interpretation (P3, confirmed)

`ForLoopValue` at `kirin-scf/src/interpret_impl.rs:10-15` returns `Option<bool>` from `loop_condition`, where `None` implicitly means "unknown." The `For` interpreter at line 199 only handles `Some(true)`, meaning `None` silently terminates the loop. For abstract interpreters, this drops the "unknown" case without widening -- the loop body is never explored when the condition is indeterminate. This is correct for concrete execution but undocumented for abstract interpretation.

**File:** `crates/kirin-scf/src/interpret_impl.rs:199`

### D3. `Lexical` vs `Lifted` share operations but lack a shared trait (P3, likely)

`kirin-function/src/lib.rs:40-55` defines `Lexical` and `Lifted` as two enum compositions sharing `FunctionBody`, `Call`, and `Return` but differing in `Lambda` vs `Bind`. In the literature (e.g., Appel's closure conversion), these are related by a program transformation. There is no trait or type-level witness of the Lambda-to-Bind transformation, meaning the relationship is informal. This is acceptable for now but may matter when adding closure conversion passes.

**File:** `crates/kirin-function/src/lib.rs:40-55`

### D4. `CheckedDiv` for floats always returns `Some` -- naming mismatch (P3, confirmed)

`kirin-arith/src/checked_ops.rs:33-36`: float `CheckedDiv` returns `Some(self / rhs)` even for division by zero (producing infinity/NaN). The trait name "checked" implies fallibility, but the float impl is infallible. The comment explains why, but the trait contract is ambiguous. Consider `TotalDiv` or documenting that "checked" means "non-panicking" rather than "error-detecting."

**File:** `crates/kirin-arith/src/checked_ops.rs:33-36`

## Summary

The dialect crates compose cleanly via the `#[wraps]` pattern and the generic `T: CompileTimeValue` parameterization. MLIR alignment is strong (cf/scf/function decomposition). The main formalism concern is D1: comparison operations conflate operand and result domains, which limits abstract interpretation precision. D2-D4 are documentation-level issues.
