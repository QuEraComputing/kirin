# Derive (Small) -- PL Theorist Formalism Review

## Findings

### DS1. `Interpretable` derive requires all-`#[wraps]` -- correct but restrictive encoding (P3, confirmed)

`kirin-derive-interpreter/src/interpretable.rs:52-66` validates that every variant must have `#[wraps]`, rejecting enums with mixed wrapper/leaf variants. This enforces that `Interpretable` dispatch is purely structural delegation -- no variant carries its own interpretation logic. This is a sound restriction (interpretation is compositional), but it means leaf dialect types with custom semantics cannot participate in derived enums. Phase 1 finding P1-8 already covers extending this; from a formalism perspective, the current encoding is a *shallow embedding* where the derive only handles the structural case.

**File:** `crates/kirin-derive-interpreter/src/interpretable.rs:52-66`

### DS2. `CallSemantics` Result type unification assumes homogeneity (P2, confirmed)

`kirin-derive-interpreter/src/eval_call/generate.rs:63-79` picks the first callable wrapper's `Result` type and constrains all others to match it (`Result = #result_type`). This is a form of *type equalization* -- all callable variants in an enum must return the same result type. The alternative is an associated type computed as a *coproduct* (sum type) of all variant result types, but that would break the single-dispatch model. The current approach is correct for the common case but would silently reject legitimate heterogeneous callable compositions at derive time rather than at call sites.

**File:** `crates/kirin-derive-interpreter/src/eval_call/generate.rs:63-79`

### DS3. `Dialect` derive composes 19 trait impls atomically (P3, confirmed)

`kirin-derive-ir/src/generate.rs:238-267`: `generate_dialect` produces 14 field-iter impls, 5 property impls, builder, and marker in one pass. The individual derives (lines 56-78 of `lib.rs`) are also exposed, so users can opt in selectively. This is well-structured -- the `Dialect` derive is a *macro functor* that applies all constituent templates. No formalism concern, noting for completeness.

**File:** `crates/kirin-derive-ir/src/generate.rs:238-267`

### DS4. `RenderDispatch` derive is minimal with no validation (P3, likely)

`kirin-derive-prettyless/src/lib.rs` at 129 total lines delegates entirely to `generate::generate`. The derive has no validation that the stage enum variants actually implement `PrettyPrint`. Misuse would produce downstream compile errors from generated code rather than a clear derive-time diagnostic. This is a minor DX issue, not a formalism one.

**File:** `crates/kirin-derive-prettyless/src/lib.rs:8-15`

## Summary

The derive crates faithfully encode the compositional structure of the trait system. DS2 is the most notable formalism finding: the Result-type equalization in `CallSemantics` derives is a pragmatic but implicit constraint. The all-`#[wraps]` requirement (DS1) is a clean restriction that separates structural dispatch from semantic implementation. Overall, the derive layer is a well-executed *generic programming* facility.
