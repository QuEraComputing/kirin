# Reviewer Prompt Templates

Each reviewer gets a prompt constructed from: their persona content + focus areas below + design context from AGENTS.md + confidence/severity from `confidence-and-severity.md`.

All prompts share this skeleton:
```
You are reviewing <crate> as the <Role> Reviewer.

[paste persona content from team directory]

## Files to Review
[file list from plan]

## Focus Areas
[role-specific — see below]

## Design Context
[paste relevant AGENTS.md convention sections]

## Confidence & Severity
[paste from confidence-and-severity.md]

## Output
[severity] [confidence] finding description — file:line
Save report to: <review-dir>/<datetime>/<crate>/<role>-<title>.md
```

---

## Formalism Reviewer Focus Areas

(a) **Abstraction composability** — do components compose cleanly? Can new capabilities be added without breaking existing code?

(b) **Literature alignment** — do definitions match established literature? Cite specific references. If the project diverges, is the divergence justified?

(c) **Syntax/API/semantic ambiguity** — are there multiple valid interpretations? Could a user reasonably misunderstand the semantics?

**Mandate:** For each significant finding, propose 2-3 alternative formalisms with a concrete metrics comparison table (downstream bound count, extensibility cost, compile-time impact, conceptual complexity). Use formal logic, PL theory, or math. Cite external references.

---

## Code Quality Reviewer Focus Areas

(a) **Clippy workaround investigation** — find every `#[allow(...)]`, `#[expect(...)]`, `dead_code` annotation. For EACH: location, root cause, removable? fix?

(b) **Duplication analysis** — identify duplicated logic. Show locations, suggest abstraction, estimate lines saved.

(c) **Rust best practices** — missing `#[must_use]`, unnecessary allocations, non-idiomatic ownership, missing `Debug`/`Display` impls. Reference the `rust-best-practices` skill.

**Clippy format:** `[severity] [confidence] #[allow(lint_name)] at file:line — root cause: <X>. Removable: yes/no. Fix: <description>.`

---

## Ergonomics/DX Reviewer Focus Areas

(a) **User repetition** — are users forced to repeat themselves? Count instances, show examples.

(b) **Lifetime complexity** — categorize: (i) hidden by derive (acceptable), (ii) visible but necessary, (iii) visible and avoidable.

(c) **Concept budget** — for implementing feature X, build a table: `| Concept | Where learned | Complexity |` for at least 2 use cases.

**Mandate:** MUST test the public API in a concrete toy scenario. Trace step by step, note friction. Explore 2-3 edge cases. Report both findings AND use cases with code snippets.

---

## Dialect Author Reviewer Focus Areas (when included)

Requires domain background filled in from AGENTS.md Dialect Domain Context table.

(a) **Framework interaction** — walk through "add a new operation" step by step. Note friction: confusing attributes, unhelpful errors, boilerplate.

(b) **Domain-framework alignment** — for 2-3 key domain concepts, evaluate mapping to IR constructs. If awkward, explain the concept, current encoding, and better alternative.

(c) **Error path evaluation** — make 2-3 common mistakes, predict error messages. Flag unhelpful errors.

(d) **Incremental development** — can this dialect be built in stages (IR → parser → printer → interpreter)?

---

## Soundness Adversary Reviewer Focus Areas (when included)

(a) **Invariant inventory** — identify every invariant from asserts, docs ("must", "assumes"), implicit assumptions.

(b) **Enforcement classification** — for each: type-enforced / builder-enforced / runtime-debug / runtime-always / caller's responsibility / not enforced.

(c) **Attack construction** — for each non-type-enforced invariant: exact API call sequence, consequence (panic / corruption / UB), reachability (normal use vs adversarial).

(d) **Unsafe audit** — for `unsafe` blocks: what invariant? Enforced by whom? Can safe caller trigger UB?

**Note:** Do NOT flag intentional guard panics (see AGENTS.md). DO flag panics reachable through normal API usage.

**Output format:**
```
[severity] [confidence] Title — file:line
Invariant: <what the code assumes>
Enforcement: <classification>
Attack: <API call sequence>
Consequence: <what happens>
```
