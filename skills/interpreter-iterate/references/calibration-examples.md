# Critic Calibration Examples

These are real scoring disagreements from past iterations, corrected by the human reviewer.
**Treat these as ground truth.** When you encounter a similar pattern, use the corrected score, not your initial instinct.

Each example shows: the code pattern that was misscored, the critic's original score, the correct score, and why the correct score is what it is.

---

<!-- New examples are appended below by the skill after user feedback. -->
<!-- Format:
## <Iteration N> — R<dim>: critic scored <X>, correct is <Y>
**Pattern:** <describe the code pattern or paste the key snippet>
**Location:** <file:line if known>
**Why the critic was wrong:** <the specific reasoning — what the critic failed to check or weighed incorrectly>
**What to do instead:** <how to score this pattern correctly in future>
-->

## Iteration 19 — R7: critic did not flag vestigial trait, correct is R7 ≤ 4

**Pattern:**
```rust
// in kirin-scf/src/interpreter19/interpret.rs
pub trait ScfSeam<T: CompileTimeValue>: Env {
    fn eval_if(&mut self, op: &If<T>) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;
    fn eval_for(&mut self, op: &For<T>) -> Result<Control<Self::Value, Self::Ext>, Self::Error>;
}

// in toy-lang/src/interpreter19/interp.rs — pure delegation, no custom logic
impl<'ir, V: ToyVal> ScfSeam<kirin_arith::ArithType> for MultiInterp<'ir, V> {
    fn eval_if(&mut self, op: &If<kirin_arith::ArithType>) -> ... { self.0.eval_if(op) }
    fn eval_for(&mut self, op: &For<kirin_arith::ArithType>) -> ... { self.0.eval_for(op) }
}
impl<'ir, V: AbstractToyVal> ScfSeam<kirin_arith::ArithType> for AbstractMultiInterp<'ir, V> {
    fn eval_if(&mut self, op: &If<kirin_arith::ArithType>) -> ... { self.0.eval_if(op) }
    fn eval_for(&mut self, op: &For<kirin_arith::ArithType>) -> ... { self.0.eval_for(op) }
}
```

**Location:** `crates/kirin-scf/src/interpreter19/interpret.rs:21`, `example/toy-lang/src/interpreter19/interp.rs:380,619`

**Why the critic was wrong:** The critic saw `ScfSeam` as a legitimate customization point and scored R7 = 4 without auditing whether the trait actually carries value. All user-facing implementations of `ScfSeam` (the two newtype impls in toy-lang) are pure delegation — `self.0.eval_if(op)` with zero added logic. Behavioral distinctness = 0% for the newtype impls. Net impl economy: saves 1 impl block total. The trait is vestigial — a leftover from the seam-trait design philosophy of interpreter-17 that was not removed when the design moved away from seam traits.

Additionally, the comment on the trait declaration says `// crate-private` but the trait is `pub` — a factual inconsistency the critic also missed.

**What to do instead:** Apply the vestigial trait pre-check (see critic-brief.md R7 pre-check). When all user-facing impls of a trait are pure delegation (`self.field.method(args)`), the trait provides no customization value and should be flagged as a finding. Score R7 ≤ 4 and file a Medium finding proposing to replace the trait with direct `Interpretable` impls on the concrete types. Also flag any comment claiming visibility that contradicts the actual `pub`/`pub(crate)` annotation.

---

## Iteration 19 — R10: critic did not flag expression boilerplate, correct is R10 ≤ 3

**Pattern:**
```rust
Ok(Control::Ext(CursorExt::Push(cursor.lift())))
```

**Location:** Appears **113 times across 42 files** — `crates/kirin-scf/src/interpreter*/interpret.rs`, `crates/kirin-scf/src/interpreter*/cursor.rs`, `crates/kirin-interpreter-*/src/*.rs`, `example/toy-lang/src/interpreter*.rs`.

**Why the critic was wrong:** The critic scored R10 by checking file size and module hierarchy only. It did not search for repeated expression patterns. This expression has two compounding problems:

1. **DRY violation at the expression level** — `Control::Ext(CursorExt::...)` states the concept "extension/cursor slot" twice: once in the `Control::Ext` variant name and once in the `CursorExt` type name. The programmer must write "Ext" twice to express one intent; a change to the relationship between these two types requires changes at both names in every call site. This is a textbook DRY violation.

2. **Repetition without shorthand** — 113 occurrences of the same 4-level wrapping expression with no constructor shorthand. A method `Control::push(cursor)` would collapse this to one token at every call site.

**What to do instead:** Apply the R10 boilerplate pre-check (see critic-brief.md R10 pre-check). Search for constructor chains of depth ≥ 3 that appear 5+ times. When found, score R10 ≤ 3 (≥ 10 occurrences without a shorthand is automatic R10 ≤ 3) and file a High severity finding. For this specific pattern, the finding should propose: (a) rename `Control::Ext` to eliminate the DRY violation with `CursorExt`, and (b) add a `Control::push(cursor: impl Lift<C>) -> Self` constructor so call sites collapse from 4 levels to 1.

---

## Iteration 19 — R10: critic did not flag suppressed clippy lint, correct is R10 finding (Medium)

**Pattern:**
```rust
impl<V: Clone, L: Dialect> ForCursor<V, L> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(iv: V, end: V, step: V, carried: V, body: Block,
               body_stage: CompileStage, init_arg_count: usize,
               results: Vec<ResultValue>) -> Self { ... }
}
```

**Location:** `crates/kirin-scf/src/interpreter19/cursor.rs` — `ForCursor::new`

**Why the critic was wrong:** The critic did not scan for `#[allow(clippy::...)]` annotations. `too_many_arguments` is a real structural problem: 8 positional arguments make call sites unreadable and fragile to argument reordering. The `allow` attribute silences the linter but does not fix the underlying API design issue. The correct fix is `#[bon::builder]` on the `impl` block plus `#[builder]` on `new`, which gives callers named fields and eliminates the arity problem entirely.

**What to do instead:** Apply the R10 suppressed lint audit (pre-check item 4). When `#[allow(clippy::too_many_arguments)]` appears on a constructor or function, file a Medium R10 finding. The suggestion must be: add `#[bon::bon]` on the `impl` block and `#[builder]` on the function, remove the `#[allow]`. Update all call sites to use the builder API (`Type::builder().field(v)....build()`). Do not accept the `allow` annotation as a legitimate fix.

---

## Iteration 16 — R3: critic did not flag as critical, correct is R3 = 1

**Pattern:**
```rust
pub trait AbstractEnv: Env {
    fn enqueue_block(&mut self, block: Block, args: Vec<Self::Value>);
    fn record_return(&mut self, v: Self::Value) -> Result<(), Self::Error>;
    fn current_function(&self) -> SpecializedFunction;

    /// Maximum number of abstract `scf.for` loop unrolling iterations.
    fn for_widening_budget(&self) -> usize {
        10
    }
}
```

**Location:** `crates/kirin-interpreter-16/src/env.rs` (approximately)

**Why the critic was wrong:** `for_widening_budget` encodes `scf.for`-specific semantics (loop widening budget) directly on the core `AbstractEnv` framework trait. This is SCF dialect logic leaking into the interpreter crate. Every implementor of `AbstractEnv` — including implementations that have nothing to do with SCF — must now carry this method. The critic likely saw a small, innocuous-looking default method and did not recognize that its name and semantics are dialect-specific.

**What to do instead:** Any trait method whose name or semantics reference a specific dialect op (`scf.for`, `cf.br`, etc.) is a critical R3 violation regardless of whether it has a default impl. Score R3 = 1. The widening budget belongs in the SCF dialect's own cursor or config struct, passed through the user-defined cursor state — not on the framework env trait.
