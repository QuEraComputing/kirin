# Interface Ergonomics Review — 2026-03-12

**Scope:** Full workspace — deep focus on interface ergonomics
**Reviewers:** PL Theorist, Physicist/DSL User, Rust Engineer
**Plan:** docs/plans/2026-03-12-ergonomics-review-plan.md

## Abstractions & Type Design

[P2] [confirmed] [Accepted] Duplicate `I::Error: From<InterpreterError>` bound appears on both the impl block and the `interpret` method in every manual `Interpretable` impl. The method-level bound (required by trait signature) makes the impl-level bound redundant. 6 redundant duplications in kirin-function (verified). — crates/kirin-function/src/interpret_impl.rs:27,36 [PL Theorist, Rust Engineer]

[P3] [confirmed] [RESOLVED — already fixed] `#[wraps]` + Region/Block limitation was documented as a codegen artifact but is no longer present. Verified: toy-lang uses `#[wraps]` on `Lexical` (Region) and `StructuredControlFlow` (Block) successfully. Roundtrip tests converted from inlined to `#[wraps]` delegation — all pass. AGENTS.md and stale comments updated. — crates/kirin-derive-chumsky (codegen) [PL Theorist]

[P3] [likely] [Kept] `Interpretable<'ir, I>: Dialect` requires the full 17+ supertrait `Dialect` bound. Most interpretable types only need a subset. Practical cost is near zero (derive handles it), but forecloses `Interpretable` on lightweight non-Dialect types. — crates/kirin-interpreter/src/interpretable.rs [PL Theorist]

## API Ergonomics & Naming

[P1] [confirmed] [Accepted — #[non_exhaustive] + __Phantom approach] PhantomData boilerplate: every generic dialect struct/enum requires `marker: PhantomData<T>` + `#[kirin(default)]`. Repeated ~25 times across dialect crates. User chose: reduce to one `#[doc(hidden)] __Phantom(PhantomData<T>)` per enum with `#[non_exhaustive]` to hide it from external matchers. — crates/kirin-arith/src/lib.rs:94,103,110,120,128,136 [Physicist, Rust Engineer]

[P2] [confirmed] [Accepted — rename to `builders`] `#[kirin(fn)]` is cryptic shorthand — reads as "this is a function" rather than "generate builder/constructor functions." User chose to rename to `#[kirin(builders)]`. — crates/kirin-arith/src/lib.rs:85 [Physicist]

[P3] [confirmed] [Kept] Attribute namespace proliferation (3 namespaces at toy-lang, ~5 project-wide) lacks a discovery path. No single reference maps attributes to derives. Downgraded from P2 after verification. — example/toy-lang/src/language.rs:21-23 [Physicist, Rust Engineer] [Downgraded by verification]

[P2] [likely] [Accepted — deferred] Arith-to-function complexity cliff: writing kirin-arith is ~138 LOC, kirin-function is ~670 LOC across 8 files (verified). No intermediate stepping stone. User accepted but noted: "will come back to improve examples later, not yet." — crates/kirin-function/src/interpret_impl.rs [Physicist]

[P3] [confirmed] [Kept] Inconsistent result field naming: `result` in Arith vs `res` in Call/Lambda. Minor but hinders intuition-building for new dialect authors. — crates/kirin-arith/src/lib.rs:92 vs crates/kirin-function/src/call.rs:9 [Physicist]

## Code Quality & Idioms

[P2] [confirmed] [RESOLVED — already fixed] No compile-time guidance for #[wraps] + Region/Block E0275. Investigation revealed the E0275 issue no longer exists — `#[wraps]` works with Region/Block types. Stale documentation and test comments updated. — crates/kirin-derive-chumsky (codegen) [Rust Engineer]

[P3] [confirmed] [Kept] Attribute typo detection: no validation catches `#[kirin(format = ...)]` (should be `#[chumsky(format = ...)]`). A `deny(unknown_attribute)` style check within each derive would improve DX. — crates/kirin-arith/src/lib.rs:88 [Rust Engineer]

## Cross-Cutting Themes

1. **PhantomData boilerplate** — identified by 2 reviewers (Physicist, Rust Engineer) across API Ergonomics & Code Quality. Highest-impact single improvement. Resolution: `#[non_exhaustive]` + single `__Phantom` variant.
2. **#[wraps] + Region/Block** — identified by 2 reviewers (PL Theorist, Rust Engineer) across Abstractions & Code Quality. Resolution: **already fixed** — stale documentation updated, roundtrip tests converted to use `#[wraps]`.
3. **Redundant where-clause bounds** — identified by 2 reviewers (PL Theorist, Rust Engineer) across Abstractions & Code Quality. Resolution: remove redundant impl-level bounds.
4. **Attribute namespace complexity** — identified by 2 reviewers across API Ergonomics & Code Quality. Resolution: kept as informational, plus typo detection improvement.

## Summary

- P0: 0 issues
- P1: 1 issue (PhantomData boilerplate) — **Accepted**
- P2: 4 issues — 3 **Accepted** (bounds, naming, #[wraps] root cause fix), 1 **Deferred** (complexity cliff)
- P3: 5 notes — all **Kept**

Confirmed: 8 | Likely: 2 | Uncertain: 0

## Action Items

| Priority | Finding | Action | Status |
|----------|---------|--------|--------|
| P1 | PhantomData boilerplate | Refactor to `#[non_exhaustive]` + `__Phantom` per enum | Done |
| P2 | `#[kirin(fn)]` → `#[kirin(builders)]` | Rename attribute in derive-ir + update all consumers | Done |
| P2 | Duplicate where-clause bounds | Remove redundant impl-level `I::Error: From<InterpreterError>` | Done |
| ~P2~ | ~#[wraps] + Region/Block E0275~ | Already fixed — stale docs updated, tests converted | Done |
| P2 | Complexity cliff | Create intermediate example dialect | Deferred |

<details>
<summary>0 findings filtered (click to expand)</summary>

No findings were filtered — all findings were consistent with AGENTS.md design context.
</details>
