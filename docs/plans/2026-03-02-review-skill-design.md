# Review Skill Design

**Date:** 2026-03-02
**Scope:** New `/review` skill for comprehensive codebase review with themed reports and selectable reviewer personas

## Skill Identity

**Name:** `/review` at `.agents/skills/review/SKILL.md`

**Triggers:**
- **Manual:** `/review <scope>` (e.g., `/review kirin-ir`, `/review interpreter`, `/review recent`, `/review full`)
- **Post-refactor:** Auto-suggested when `/refactor` completes Phase 4
- **Periodic:** Auto-suggested when 10+ commits accumulate on a feature branch since last review

**Scope types:**
- `full` — entire workspace
- `<crate-name>` — single crate (e.g., `kirin-ir`, `kirin-chumsky`)
- `<subsystem>` — logical subsystem spanning crates (e.g., `interpreter`, `derive`, `parser`)
- `recent` — changes since last review or last merge to main (via `git diff`)

## Reviewer Pool

Four review-eligible personas from `.agents/team/`:

| Reviewer | Expertise | When to include |
|---|---|---|
| PL Theorist | Formalism, abstraction design, trait boundaries, type-level invariants | Trait design, type-level patterns, abstraction boundaries |
| Compiler Engineer | Build graph, error quality, scalability, dispatch efficiency | Performance, build graph, error quality, scalability |
| Rust Engineer (Implementer) | Code quality, idioms, safety, patterns | Code quality, idioms, safety, patterns |
| Physicist | API clarity, naming, learning curve, composability | API clarity, naming, learning curve, composability |

Default: all four for `full` scope; auto-select relevant subset for narrower scopes.

## Two-Phase Process

### Phase 1: Review Plan

Saved to `docs/plans/YYYY-MM-DD-<scope>-review-plan.md`.

1. **Scope analysis** — identify files in scope, count lines, map module structure
2. **Select reviewers** — propose reviewer roster with rationale based on scope content; user approves/adjusts
3. **Define review themes** — propose which themed sections apply to this review:
   - Correctness & Safety
   - Abstractions & Type Design
   - Performance & Scalability
   - API Ergonomics & Naming
   - Code Quality & Idioms
4. **Assign themes to reviewers** — each theme gets a primary reviewer (by expertise) + optional secondary

### Phase 2: Execute Review

Report saved to `docs/reviews/YYYY-MM-DD-<scope>-review.md`.

1. **Dispatch reviewer subagents in parallel** — each reviewer gets assigned themes + files in scope, using `dispatching-parallel-agents`
2. **Collect findings** — each reviewer produces findings tagged with theme, severity (P0-P3), file:line references
3. **Synthesize report** — organize findings by theme (not by reviewer); within each theme: P0 first, then P1, P2, P3; attribution shown inline (e.g., "[Compiler Engineer]")
4. **Cross-cutting themes** — identify patterns appearing across multiple themes/reviewers
5. **Commit report** to `docs/reviews/`

## Report Format

```markdown
# <Scope> Review — YYYY-MM-DD

**Scope:** <description of what was reviewed>
**Reviewers:** <list>
**Plan:** docs/plans/YYYY-MM-DD-<scope>-review-plan.md

## Correctness & Safety
[P0] <finding> — <file:line> [Reviewer]
[P1] <finding> — <file:line> [Reviewer]

## Abstractions & Type Design
[P1] <finding> — <file:line> [PL Theorist]
...

## Cross-Cutting Themes
1. <theme> — identified by <N> reviewers across <themes>

## Summary
- P0: N issues (must fix)
- P1: N issues (should fix)
- P2: N improvements (nice to have)
- P3: N notes (informational)
```

## Shared Team Directory

Move persona files from `.agents/skills/refactor/team/` to `.agents/team/`. Both `/refactor` and `/review` reference them.

Review-eligible personas:
- `.agents/team/pl-theorist.md`
- `.agents/team/compiler-engineer.md`
- `.agents/team/implementer.md` (acts as "Rust Engineer" in review context)
- `.agents/team/physicist.md`

Non-review personas (used by `/refactor`):
- `.agents/team/guardian.md`
- `.agents/team/migrator.md`
- `.agents/team/documenter.md`

## Integration

**Skills this skill uses:**
- `dispatching-parallel-agents` — run reviewer subagents concurrently
- Persona files from `.agents/team/` — reviewer role definitions

**Skills that reference this skill:**
- `/refactor` Phase 4 — auto-suggests `/review` after refactor completes
- `/finishing-a-development-branch` — could auto-suggest `/review recent` before merge

**What this skill does NOT do:**
- No code changes (read-only)
- No implementation planning (that's `/writing-plans`)
- No PR-level review (that's `/requesting-code-review`)
- No fixing issues found (user decides what to act on)

## Subsystem Mapping

| Subsystem | Crates |
|---|---|
| `interpreter` | kirin-interpreter, kirin-derive-interpreter |
| `parser` | kirin-chumsky, kirin-chumsky-derive, kirin-chumsky-format |
| `derive` | kirin-derive-core, kirin-derive, kirin-chumsky-derive, kirin-derive-interpreter, kirin-prettyless-derive |
| `ir` | kirin-ir |
| `dialects` | kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-cmp, kirin-function |
| `printer` | kirin-prettyless, kirin-prettyless-derive |
