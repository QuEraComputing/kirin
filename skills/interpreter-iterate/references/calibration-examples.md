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
