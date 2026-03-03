# /refactor Skill Design

## Goal

A Rust-specific refactoring orchestration skill that wraps existing execution infrastructure (executing-plans, subagent-driven-development) with architectural guardrails and a configurable agent team. Eliminates the recurring friction of planning loops, wrong crate placements, and batched-at-end compilation failures.

## Skill Chain Position

```
brainstorming → writing-plans → /refactor → (execution via staffed team)
```

`/refactor` sits between the plan and execution. It adds what the execution skills lack:
- Architectural pre-flight with user approval
- Exploration budget scaled by refactor size
- Per-refactor agent team staffing from a persistent roster
- Compilation gates injected into all agents

## Trigger

- **Explicit**: user invokes `/refactor`
- **Auto-suggest**: Claude suggests it when detecting cross-crate moves, trait extractions, module splits, or renames across 3+ files

## Team Architecture

### Persistent Roster

Stored in `.agents/skills/refactor/team/`. Each role is a markdown file containing a full persona prompt: background, perspective, responsibility, and what they look for.

| Role | File | Background & Perspective |
|------|------|-------------------------|
| Guardian | `guardian.md` | Systems architect focused on structural integrity. Verifies crate ownership, visibility boundaries, feature flags, dependency graphs. Bookends the refactor with pre-flight and post-validation. Pre-flight produces a migration checklist (which crates, which imports/bounds/call sites change) that the Migrator executes. |
| Implementer | `implementer.md` | Senior Rust developer with deep compiler engineering expertise. Knows best practices for IR design, pass infrastructure, trait-based dispatch, and layered crate architectures. Makes code changes with discipline: cargo check after every file, never removes wrappers without checking visibility bridging, follows CLAUDE.md conventions. |
| Migrator | `migrator.md` | Downstream integration specialist. Executes the migration checklist produced by the Guardian: updates imports, trait bounds, call sites, derive attributes across dependent crates. Does not do its own analysis of what needs changing — follows the Guardian's checklist mechanically. |
| PL Theorist | `pl-theorist.md` | Programming languages researcher. Reviews formalism and abstraction design. Are trait boundaries clean? Are type-level invariants sound? Is the encoding principled or ad-hoc? Thinks in terms of parametricity, coherence, and compositionality. |
| Compiler Engineer | `compiler-engineer.md` | Compiler infrastructure pragmatist. Reviews practical engineering: compilation time, error message quality, derive macro ergonomics, build graph impact. Asks "will this scale?" and "will error messages be helpful?" |
| Physicist | `physicist.md` | Experimental physicist building a DSL to control optical tweezer arrays. Does not care about implementation details. Core responsibility: help develop clear API definitions, easy-to-understand concepts, intertwined abstractions, and a smooth learning curve. Reviews API clarity, concept naming, and whether abstractions compose intuitively for a domain scientist. Uses concrete use cases from their tweezer control work to ground review comments when applicable — "if I'm trying to do X, this naming/abstraction is confusing because..." |
| Documenter | `documenter.md` | Technical writer who maintains CLAUDE.md, AGENTS.md, design docs, and auto-memory. Updates conventions and public API documentation when refactors change them. |

### Role Interaction Rules

- **Guardian → Migrator**: Guardian's pre-flight produces a migration checklist. Migrator executes it, does not do independent analysis.
- **Implementer ↔ Compiler Engineer**: Same compiler engineering expertise, different roles. Implementer applies it while writing code, Compiler Engineer applies it while reviewing. Separation is doer vs critic.
- **PL Theorist ↔ Physicist**: Intentional tension. PL Theorist pushes for principled abstractions, Physicist pushes for intuitive usability. When they disagree, the disagreement surfaces to the user for a decision — they do not resolve it themselves.

### Per-Refactor Staffing

Not all roles activate for every refactor. The skill asks questions about refactor scope and proposes a roster:

**Staffing heuristics:**
- Simple rename across crates → Implementer + Migrator + Compiler Engineer
- Trait extraction to new crate → Guardian + Implementer + Migrator + Compiler Engineer
- Public API redesign → Guardian + Implementer + Migrator + full Review Panel (PL Theorist + Compiler Engineer + Physicist) + Documenter
- Module split within one crate → Implementer + Compiler Engineer
- Convention change → all roles

User confirms or adjusts the proposed roster before execution begins.

### Review Panel

The three reviewers (PL Theorist, Compiler Engineer, Physicist) can be staffed individually or as a debate panel:
- **Individual**: for focused feedback on one dimension (e.g., just engineering review)
- **Panel**: for API/trait redesigns where multiple perspectives matter — they debate and converge on recommendations

### Template Growth

After each successful refactor, the skill asks: "Save this team configuration as a template?"

Templates saved to `.agents/skills/refactor/team/templates/<name>.md` containing:
- Refactor type and scope
- Staffed roles and why
- What worked
- What to adjust next time

These templates inform future staffing suggestions — institutional memory for agent team patterns.

## Four Phases

### Phase 1: Scope & Pre-flight

**Exploration budget (hard cap):**
- Small refactors (1-3 crates): 10 file reads + 5 grep searches
- Large refactors (4+ crates): 20 file reads + 15 grep searches

After budget is spent, Claude MUST have a concrete plan and proceed to staffing.

**Pre-flight checklist (Guardian or lead agent):**
- For each moved/created type: target crate, feature flag, visibility level
- Identify dependent crates via Cargo.toml dependency graph
- Check for visibility-bridging wrappers in affected code
- List all `pub` items that will change
- Present architectural summary → **user approves before any code changes**

### Phase 2: Team Staffing

Claude asks about:
- What's changing (traits, modules, types, renames)
- Which crates are affected
- Mechanical (rename/move) vs semantic (API redesign)?
- Downstream API impact?

Proposes a roster from the persistent team, with rationale for each staffed role. Checks templates for similar past refactors. User confirms or adjusts.

### Phase 3: Guarded Execution

**Invariants injected into all staffed agents:**
- `cargo check -p <crate>` after every file modification (not batched at end)
- Never use `#[allow]` or ignore comments as fixes
- Never remove one-liner wrappers without verifying visibility bridging
- Never place new types/traits without checking CLAUDE.md crate ownership
- `cargo nextest run --workspace` before any commit

**Execution** delegates to subagent-driven-development or executing-plans, with staffed roles mapped to subagent prompts from the team/ persona files.

### Phase 4: Review + Documentation

- Staffed reviewers run (individual or panel, as determined in Phase 2)
- Documenter updates CLAUDE.md/AGENTS.md/memory if conventions changed
- Guardian runs final workspace validation (`cargo build --workspace` + `cargo nextest run --workspace`)
- Hand off to finishing-a-development-branch

## File Structure

```
.agents/skills/refactor/
├── SKILL.md                          # Skill definition (phases, trigger, flow)
├── team/
│   ├── guardian.md                    # Role persona: systems architect
│   ├── implementer.md                # Role persona: senior Rust dev
│   ├── migrator.md                   # Role persona: integration specialist
│   ├── pl-theorist.md                # Role persona: PL researcher
│   ├── compiler-engineer.md          # Role persona: compiler pragmatist
│   ├── physicist.md                   # Role persona: DSL user / domain scientist
│   ├── documenter.md                 # Role persona: technical writer
│   └── templates/                    # Saved team configurations
│       └── (grows over time)
```

## Friction Points Addressed

| Friction | How /refactor addresses it |
|----------|---------------------------|
| Planning loops (16-37 min without code) | Exploration budget: hard cap on reads/greps, then must execute |
| Wrong crate placement | Guardian pre-flight verifies placement → user approves |
| Batched compilation failures | Invariant: cargo check after every file change |
| Removed visibility-bridging wrappers | Invariant: never remove wrappers without checking |
| Ignore comments as fixes | Invariant: never use #[allow] or ignore comments |
| Missing architectural context | Team persona files encode project-specific knowledge |
| One-size-fits-all agent teams | Per-refactor staffing from persistent roster |

## Integration

**Required workflow skills:**
- `writing-plans` — creates the plan /refactor executes
- `executing-plans` or `subagent-driven-development` — execution delegation
- `finishing-a-development-branch` — completion after Phase 4

**Optional:**
- `brainstorming` — upstream design work before /refactor
- `simplify` — post-refactor code cleanup
