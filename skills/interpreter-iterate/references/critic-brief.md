# Critic Brief

The critic must produce a **structured report** with three parts: (1) a rubric scorecard, (2) strengths worth preserving, (3) per-finding review notes. The critic reads code, does not write any.

**Before scoring anything:** Review the calibration examples appended after this brief. These are real scoring disagreements from past iterations corrected by the human reviewer. They are ground truth — when you encounter a similar pattern, use the corrected score. Do not re-derive scores that have already been calibrated.

## Part 1 — Rubric Scorecard

Score each dimension 1–5 using the rubric table below. A score of 5 means fully satisfied; 1 means critically broken. Use the full range: **do not default to 3**. If the code fully satisfies a criterion with no caveats, assign 5. If there is a critical gap, assign 1. Scores of 2 and 4 are valid and expected for partially-met criteria.

**Score each dimension independently.** Do not let your overall impression of the design or a high score on one dimension bias others. In particular: R2 (API symmetry) measures *uniformity of the lift/project principle across all boundaries*; R7 (elegance) measures *coherence and predictability of the overall algebra* — these are related but distinct. Similarly, R3 (dialect locality) measures *whether the interpreter crate needs changes*; R5 (ergonomics) measures *how many concepts a dialect author must import and implement* — locality is necessary but not sufficient for ergonomics.

**For each dimension, before assigning a score: cite 2–3 specific file:line locations as evidence.** This grounds the score in the code rather than impressions. If you cannot find code evidence, that itself informs the score.

**R3 mandatory pre-check (run before scoring R3):** Dialect leakage is the most commonly missed critical violation. Before scoring R3, explicitly check all of the following in the interpreter crate's source (not the dialect crates or toy-lang):

1. **Dependency audit**: Read `crates/kirin-interpreter-<N>/Cargo.toml`. Any dependency on `kirin-scf`, `kirin-cf`, `kirin-function`, `kirin-arith`, or any other dialect crate is an automatic R3 = 1.
2. **Import scan**: Search for `use kirin_scf`, `use kirin_cf`, `use kirin_function` in all `.rs` files under `crates/kirin-interpreter-<N>/src/`. Any import is R3 ≤ 2.
3. **Trait method audit**: Read every method in every `trait` defined in the interpreter crate. Any method whose signature mentions a dialect-specific type (`Block` from SCF, `ScfIf`, `ForRegion`, etc.) or whose name encodes dialect knowledge (`eval_for`, `eval_if`, `for_widening_budget`, `enqueue_scf_block`) is leakage. Example of a critical violation: a method `fn for_widening_budget(&self) -> usize` on `AbstractEnv` — the widening budget for `scf.for` is SCF semantics and belongs in `kirin-scf`, not in the interpreter framework trait.
4. **Control flow pattern-match scan**: Search for `match` expressions in the interpreter crate that discriminate on dialect-specific variant names. Any such match is R3 = 1.

If any of items 1–4 are found, R3 must be scored 1 or 2 regardless of how clean the rest of the design looks. Do not assign R3 ≥ 3 without having explicitly checked all four items.

| # | Dimension | 5 (Excellent) | 3 (Acceptable) | 1 (Critical gap) |
|---|-----------|--------------|----------------|-----------------|
| R1 | **Requirement completeness** | All non-negotiable features present and tested: concrete + abstract, single + multi-stage, SCF, cross-stage calls, both entry-point use cases, forward + backward AI, sparse AI | Most features present; one variant (direction, sparsity, or entry) missing or untested | A core feature (interpretation mode, SCF, entry flexibility, AI direction) is absent or broken |
| R2 | **API symmetry (Lift/Project)** | Every dialect-local ↔ total boundary has a symmetric bidirectional API (`lift`/`project`); applied uniformly across cursors, values, effects, and environments; zero-cost enum-based, no heap allocation | Symmetric API present for cursors but inconsistently applied to values or effects | Symmetry principle absent; boundaries crossed ad-hoc (downcasts, `Any`, one-way coercions) or requires heap allocation |
| R3 | **Dialect locality** | Dialect authors implement only `Interpretable<E>`; cursor types and dispatch live in user code; the interpreter crate has zero knowledge of any specific dialect — no imports from `kirin_scf`, `kirin_cf`, `kirin_function`, etc.; no pattern-matching on dialect-specific op types in the framework core | Minor leakage — one or two framework methods take a dialect-specific type as parameter, or the interpreter crate has a dialect as an optional/feature-gated dep | The interpreter crate directly imports a dialect crate (`kirin_scf` etc.), pattern-matches on dialect-specific variants, or hard-codes SCF/CF control-flow logic (e.g. branch merging, loop fixpoint) inside the interpreter driver rather than delegating via `Interpretable<E>` |
| R4 | **Mode uniformity** | `Interpretable<E>` works identically for concrete and abstract modes; forward and backward traversal are a parameter, not a structural fork; pure ops have a single generic impl covering all mode/direction combinations; mode-specific ops use `E::Mode` discriminant only where necessary | Mostly uniform; a few ops duplicated, or forward/backward require separate trait impls | Separate traits or duplicate impls for concrete vs. abstract, or backward AI requires restructuring the core cursor |
| R5 | **Dialect ergonomics** | Dialect authors write ≤ 1 impl per op type AND import ≤ 5 names from the framework prelude to get started; composition is mechanical enough to be derived; no repeated type bounds copy-paste | Moderate repetition or prelude breadth, but all discoverable from one import site | Extensive manual impl repetition OR author must hunt across multiple internal modules to understand the API contract |
| R6 | **Type-system correctness** | No `'static` bounds, no `unsafe`, no `Box<dyn Trait>` in framework APIs; `'ir` lifetime threads correctly through all borrows | `'static` used only in abstract interp pipeline borrow (known limitation, tracked) | `unsafe`, `transmute`, or unsound lifetime casts present |
| R7 | **Algebraic elegance** | Lift/Project, Mode, Cursor, and Env form a coherent algebra; naming is consistent; a new developer can predict the pattern from one example | Mostly coherent; some naming inconsistencies or ad-hoc special cases | Ad-hoc design; each new case requires a novel pattern |
| R8 | **Extensibility** | A new analysis or interpreter type can be added by implementing traits in user code only; demonstrated by at least one extensibility probe test | Framework is extensible in theory but probe not yet written | Framework requires core changes to add new interpreter types |
| R9 | **Entry point flexibility** | Fixed-source and symmetric/dynamic entry are both first-class: fixed-source is typed as `Interp<HomeDialect,...>`; symmetric exposes a dialect-agnostic entry API where any language can initiate execution; both tested | One use case is supported; the other is possible but awkward (e.g. requires type-level workarounds to change the home dialect) | Only one entry mode exists; switching entry language requires recompilation or `unsafe` casts |
| R10 | **Implementation readability** | Each file has a single clear responsibility and stays under ~150 lines; module hierarchy mirrors the conceptual hierarchy (e.g. `concrete/`, `abstract/`, `cursor/`); type names read naturally when module-qualified — `abstract::Interpreter`, `cursor::Block`, `env::Concrete` — with no redundancy (`abstract::AbstractInterpreter` fails this) and no cryptic abbreviations; single-letter type params only where the meaning is unambiguous by convention (`E`, `V`) | Most files are focused; one or two files exceed ~200 lines or mix concerns; qualified names mostly read well but a few are redundant or abbreviated; module hierarchy is flat where nesting would help | Implementation is largely in one or two giant files (500+ lines); modules are not organized by concept; type names are either redundant with their module or so abbreviated they require context to decode |

**Score 4** = criterion mostly met with one clear caveat (e.g. one boundary missing symmetry, one internal type leaking, one workaround needed). **Score 2** = criterion attempted but fundamentally insufficient (e.g. lift/project exists only for cursors and is structurally incompatible with values, or unsafe is isolated to one place but load-bearing).

**Overall iteration grade** = average of R1–R10, rounded to one decimal. Report it prominently.

## Part 2 — Strengths Worth Preserving

For each design decision or pattern in this iteration that scores well or represents a genuine insight, write a strength note. These are things future iterations should consider keeping or adapting — not because they're mandated, but because they represent hard-won solutions that shouldn't be accidentally discarded when switching design stances.

```
Strength #<K>
Dimension: R<N> — <dimension name>
Location: <file>:<line> (or range)
Pattern: <name the pattern or abstraction — e.g. "mode discriminant via PhantomData marker", "Lift as a free conversion typeclass">
What it achieves: <why this works well — what problem it elegantly solves, cited against the rubric>
Portability: <how transferable is this to a different design stance — freely portable | stance-dependent | tightly coupled to this design>
```

Strengths must be as specific as findings — cite code, not impressions. A strength that can't be pointed to in the source isn't a strength, it's a vibe.

The number of strengths should reflect reality: a score-3 iteration might have 1–2 genuine strengths; a score-5 iteration might have 4–6. Do not inflate or deflate to seem balanced.

## Part 3 — Per-Finding Review Notes

For each issue found (severity ≥ Medium), write a structured finding:

```
Finding #<K>
Dimension: R<N> — <dimension name>
Severity: Critical | High | Medium | Low
Location: <file>:<line> (or range)
Observation: <what the code does — factual, no editorializing>
Problem: <why this violates the rubric or requirements — cite the specific rubric cell>
Suggestion: <concrete, actionable change — specific enough that an implementer can act without asking follow-up questions; propose the new trait signature, the new type, or the new abstraction if applicable>
Effort: <estimate: trivial | small | medium | large>
```

Findings must be grounded in the code — cite lines, not vibes. The suggestion field is required (unlike before, the critic *does* propose solutions here — specific ones). Do not suggest vague "refactor this" or "improve this"; propose the actual trait, type, or pattern change.

## Weighted Convergence Score

```
score = Σ (5 - dimension_score) * weight
```

A perfect design (all 5s) scores 0. A design with all 4s scores 1 × 33 = 33. To reach convergence (≤ 8), most dimensions must be at 5, with at most two or three low-weight dimensions at 4. Example passing score: R1=5, R2=5, R3=5, R4=5, R5=4, R6=5, R7=4, R8=5, R9=5, R10=4 → deficit = 0+0+0+0+2+0+2+0+0+3 = 7 ✓

| Dimension | Weight |
|-----------|--------|
| R1 (completeness) | 5 |
| R2 (API symmetry) | 3 |
| R3 (dialect locality) | 4 |
| R4 (mode uniformity) | 3 |
| R5 (dialect ergonomics) | 2 |
| R6 (type correctness) | 4 |
| R7 (elegance) | 2 |
| R8 (extensibility) | 3 |
| R9 (entry flexibility) | 4 |
| R10 (readability) | 3 |

**Convergence** when weighted score ≤ 8 AND R1 ≥ 4 AND R6 ≥ 4 AND R9 ≥ 4 (completeness, type correctness, and entry flexibility are never negotiable).
