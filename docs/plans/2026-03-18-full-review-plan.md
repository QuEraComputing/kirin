# Full Workspace Review Plan — 2026-03-18

## Scope

Full workspace: 311 Rust files, ~46K lines across 21 crates.

### Crate Groups for Review

**Tier 1 — Core (individual review):**
| Crate | Files | Lines | Why individual |
|-------|-------|-------|----------------|
| `kirin-ir` | 61 | 8851 | Core IR types, all abstractions root here |
| `kirin-interpreter` | 36 | 6752 | Interpreter framework, complex trait decomposition |
| `kirin-chumsky` | 32 | 5787 | Parser infrastructure, lifetime-heavy APIs |
| `kirin-derive-toolkit` | 52 | 7082 | Shared derive infrastructure |
| `kirin-derive-chumsky` | 29 | 5323 | Parser derive macros |
| `kirin-prettyless` | 22 | 3298 | Pretty printer |

**Tier 2 — Grouped review:**
| Group | Crates | Combined Lines |
|-------|--------|----------------|
| Dialects | kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-cmp, kirin-function | ~3295 |
| Derive (small) | kirin-derive-ir, kirin-derive-interpreter, kirin-derive-prettyless | ~1846 |
| Testing | kirin-test-types, kirin-test-languages, kirin-test-utils | ~2037 |
| Utilities | kirin-lexer, kirin-interval | ~2445 |

## Reviewer Roster

| Reviewer | Persona | Assigned Theme | Focus |
|----------|---------|----------------|-------|
| **PL Theorist** | `.agents/team/pl-theorist.md` | Formalism | (a) abstraction composability, (b) literature alignment, (c) semantic ambiguity. Must propose 2-3 alternative formalisms with concrete comparison metrics. |
| **Implementer** (Rust Engineer) | `.agents/team/implementer.md` | Code Quality | (a) clippy workaround audit (`#[allow(...)]`), (b) logic duplication, (c) Rust best practices |
| **Physicist** (DSL User) | `.agents/team/physicist.md` | Ergonomics/DX | (a) repetition in user code, (b) lifetime complexity, (c) concept budget analysis. Must try API in toy scenarios. |
| **Compiler Engineer** | `.agents/team/compiler-engineer.md` | Cross-cutting | Build graph health, error message quality, scalability concerns — feeds into all three themes |

## Review Phases

### Phase 2a: Parallel Per-Crate Review
Each reviewer examines each Tier 1 crate independently. Tier 2 groups get reviewed as units.
**10 review units × 4 reviewers = 40 review reports** (parallelized by crate group)

Output: `docs/review/2026-03-18/<crate>/<role>-<title>.md`

### Phase 2b: Cross-Review
Each reviewer reads the other reviewers' reports for the same crate and identifies false positives / low priorities.

### Phase 2c: Lead Aggregation
Per-crate lead reviewer synthesizes a `final-report.md` for each crate/group.

### Phase 2d: Main Report
Main lead aggregates all final-reports into `docs/review/2026-03-18/report.md`.

## Design Context (for reviewers)

Relevant AGENTS.md sections to include in prompts (to prevent false positives):
- IR Design Conventions (Block vs Region, terminator caching)
- Interpreter Conventions (trait decomposition, `'ir` lifetime, `L` on method)
- Derive Infrastructure Conventions (darling re-export, `#[wraps]`, Layout pattern)
- Chumsky Parser Conventions (single lifetime, ParseDispatch, ParseEmit)
- Test Conventions (roundtrip placement, test crate ownership)
