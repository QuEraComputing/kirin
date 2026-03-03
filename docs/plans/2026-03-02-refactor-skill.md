# /refactor Skill Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create the `/refactor` skill with SKILL.md, 7 role persona files, and team template infrastructure.

**Architecture:** A skill directory at `.agents/skills/refactor/` containing the main SKILL.md (orchestration flow with 4 phases) and a `team/` subdirectory with persona markdown files for each role. Role files follow the same prompt template pattern as `subagent-driven-development/implementer-prompt.md` — they're dispatched as subagent prompts. A `team/templates/` directory stores saved team configurations that grow over time.

**Tech Stack:** Markdown, YAML frontmatter, graphviz dot (for flowcharts)

**Design doc:** `docs/plans/2026-03-02-refactor-skill-design.md`

---

### Task 1: Create directory structure and SKILL.md

**Files:**
- Create: `.agents/skills/refactor/SKILL.md`
- Create: `.agents/skills/refactor/team/templates/.gitkeep`

**Step 1: Create the directories**

```bash
mkdir -p .agents/skills/refactor/team/templates
touch .agents/skills/refactor/team/templates/.gitkeep
```

**Step 2: Write SKILL.md**

Create `.agents/skills/refactor/SKILL.md` with:

**Frontmatter:**
```yaml
---
name: refactor
description: Use when refactoring Rust code across crates, extracting traits, splitting modules, or renaming across 3+ files. Also auto-suggested when detecting cross-crate moves or trait extractions.
---
```

**Body structure (follow these sections exactly):**

```markdown
# Refactor

## Overview

Rust-specific refactoring orchestration that wraps executing-plans and subagent-driven-development with architectural guardrails and a configurable agent team. Four phases: scope & pre-flight, team staffing, guarded execution, review + documentation.

**Announce at start:** "I'm using the refactor skill to orchestrate this refactoring."

## When to Use

- Explicit: user invokes `/refactor`
- Auto-suggest when detecting: cross-crate moves, trait extractions, module splits, renames across 3+ files

**Don't use for:**
- Single-file changes
- Adding new code that doesn't touch existing abstractions
- Bug fixes that don't change public APIs

## Phase 1: Scope & Pre-flight

**Exploration budget (hard cap):**
- Small refactors (1-3 crates): 10 file reads + 5 grep searches
- Large refactors (4+ crates): 20 file reads + 15 grep searches

After budget is spent, you MUST have a concrete understanding and proceed to staffing.

**Pre-flight checklist:**

1. For each moved/created type: identify target crate, feature flag, visibility level
2. Read dependent crates' Cargo.toml to map the dependency graph
3. Check for visibility-bridging wrappers in affected code (one-liner methods that bridge pub(crate) to pub)
4. List all `pub` items that will change
5. Present architectural summary to user → **user approves before any code changes**

Output format:
    ```
    ## Pre-flight Summary

    **Refactor scope:** [small/large] ([N] crates affected)

    **Changes:**
    | Item | From | To | Crate | Visibility | Feature Flag |
    |------|------|----|-------|------------|-------------|
    | TraitName | crate-a | crate-b | kirin-X | pub | interpret |

    **Dependent crates:** [list]
    **Visibility bridges to preserve:** [list or "none found"]
    **Public API changes:** [list]
    ```

## Phase 2: Team Staffing

### Available Roles

Read persona files from `./team/` directory. Each file defines a role's background, perspective, and responsibility.

| Role | File | Staff When |
|------|------|-----------|
| Guardian | `./team/guardian.md` | Any cross-crate refactor or visibility change |
| Implementer | `./team/implementer.md` | Always |
| Migrator | `./team/migrator.md` | When downstream crates are affected |
| PL Theorist | `./team/pl-theorist.md` | API/trait redesigns, new abstractions |
| Compiler Engineer | `./team/compiler-engineer.md` | Performance-sensitive changes, derive macro work |
| Physicist | `./team/physicist.md` | Public API changes, prelude changes |
| Documenter | `./team/documenter.md` | When conventions or public API surface change |

### Staffing Process

1. Check `./team/templates/` for similar past refactors
2. Ask user about refactor scope using AskUserQuestion:
   - What's changing? (traits, modules, types, renames)
   - Which crates are affected?
   - Mechanical (rename/move) vs semantic (API redesign)?
   - Downstream API impact?
3. Propose a roster with rationale for each role
4. User confirms or adjusts

### Review Panel Configuration

The three reviewers (PL Theorist, Compiler Engineer, Physicist) can be staffed:
- **Individually**: for focused feedback on one dimension
- **As a debate panel**: for API/trait redesigns where they debate and converge

When PL Theorist and Physicist disagree, surface the disagreement to the user — they do not resolve it themselves.

### Staffing Heuristics

- Simple rename across crates → Implementer + Migrator + Compiler Engineer
- Trait extraction to new crate → Guardian + Implementer + Migrator + Compiler Engineer
- Public API redesign → Guardian + Implementer + Migrator + full Review Panel + Documenter
- Module split within one crate → Implementer + Compiler Engineer
- Convention change → all roles

## Phase 3: Guarded Execution

### Invariants (inject into ALL staffed agent prompts)

```
REFACTOR INVARIANTS — these override any conflicting instructions:
1. Run `cargo check -p <crate>` after EVERY file modification. Do not batch.
2. NEVER use `#[allow(...)]` or ignore comments as fixes for real errors.
3. NEVER remove one-liner wrapper methods without verifying they are not visibility bridges
   (methods that expose pub(crate) internals through a pub interface).
4. NEVER place new types/traits without checking CLAUDE.md crate ownership conventions.
5. Run `cargo nextest run --workspace` before ANY commit.
6. If `cargo check` fails 3 times on the same error, STOP and report the issue.
```

### Execution Delegation

Delegate to one of:
- **subagent-driven-development** — for same-session, task-by-task execution with review
- **executing-plans** — for parallel session execution with batch checkpoints

Map staffed roles to subagent prompts by reading the persona file and prepending the invariants.

**Guardian** runs as lead agent (Phase 1 pre-flight + Phase 4 validation).
**Implementer** maps to the implementer subagent prompt (with invariants prepended).
**Migrator** runs after Implementer, executing the Guardian's migration checklist.
**Reviewers** run after implementation, using their persona as the review lens.
**Documenter** runs last, before final validation.

## Phase 4: Review + Documentation

1. Staffed reviewers run (individual or panel, as determined in Phase 2)
2. If review panel is active: each reviewer produces findings independently, then they debate
3. Documenter updates CLAUDE.md/AGENTS.md/memory if conventions changed
4. Guardian runs final validation:
   - `cargo build --workspace`
   - `cargo nextest run --workspace`
   - `cargo test --doc --workspace`
   - Diff `pub` items in changed files against pre-flight list — flag unintended changes
5. Hand off to finishing-a-development-branch

## Phase 5: Template Capture

After successful refactor, ask: "Save this team configuration as a template?"

If yes, save to `./team/templates/<name>.md`:
```markdown
# Template: [name]

**Refactor type:** [description]
**Scope:** [N] crates
**Staffed roles:** [list with rationale]
**What worked:** [notes]
**What to adjust:** [notes]
**Date:** [YYYY-MM-DD]
```

## Red Flags — STOP Immediately

- Planning for more than 5 minutes without having started Phase 1 pre-flight
- Rewriting the pre-flight summary more than once
- Making code changes before user approves pre-flight summary
- Implementer and Migrator editing the same file simultaneously
- `cargo check` failing 3+ times on the same error (escalate to user)
- Any agent placing types in a crate not listed in the pre-flight summary

## Integration

**Required workflow skills:**
- `subagent-driven-development` or `executing-plans` — execution delegation
- `finishing-a-development-branch` — completion after Phase 4

**Optional:**
- `brainstorming` — upstream design work before /refactor
- `writing-plans` — creates detailed plan if one doesn't exist
- `simplify` — post-refactor code cleanup
```

**Step 3: Verify frontmatter**

Run: `cargo xtask quick-validate .agents/skills/refactor`
Expected: frontmatter valid, name and description within limits

**Step 4: Commit**

```bash
git add .agents/skills/refactor/SKILL.md .agents/skills/refactor/team/templates/.gitkeep
git commit -m "feat(skills): add /refactor skill with 4-phase orchestration flow"
```

---

### Task 2: Write Guardian persona

**Files:**
- Create: `.agents/skills/refactor/team/guardian.md`

**Step 1: Write the persona file**

Create `.agents/skills/refactor/team/guardian.md`. This is a subagent prompt template (like `subagent-driven-development/implementer-prompt.md`). It should contain:

1. **Role identity**: Systems architect focused on structural integrity
2. **Background**: Deep knowledge of Rust crate architectures, visibility rules (`pub`/`pub(crate)`/private), feature flags, and dependency graphs. Understands that wrapper methods often serve as visibility bridges.
3. **Responsibilities**:
   - Pre-flight: verify crate ownership, visibility boundaries, feature flags, dependency graph
   - Produce migration checklist for the Migrator (which crates, which imports/bounds/call sites change)
   - Post-validation: full workspace build + test, verify no unintended public API changes
4. **What to look for**:
   - Types/traits placed in wrong crate per CLAUDE.md conventions
   - Visibility boundaries broken (pub(crate) internals exposed, or pub items made private)
   - Missing feature flags on optional functionality
   - One-liner wrappers that bridge visibility gaps
   - Circular dependencies introduced
5. **Output format**: Pre-flight summary table (as shown in SKILL.md Phase 1) + migration checklist
6. **Kirin-specific context**: Reference CLAUDE.md and AGENTS.md for crate ownership rules. List the crate categories: Core, Parser/Printer, Interpreter, Dialects, Derive Infrastructure, Analysis, Testing.

**Step 2: Verify file exists and reads well**

Read the file back and verify it's a coherent persona prompt.

**Step 3: Commit**

```bash
git add .agents/skills/refactor/team/guardian.md
git commit -m "feat(skills): add Guardian role persona for /refactor skill"
```

---

### Task 3: Write Implementer persona

**Files:**
- Create: `.agents/skills/refactor/team/implementer.md`

**Step 1: Write the persona file**

Create `.agents/skills/refactor/team/implementer.md`:

1. **Role identity**: Senior Rust developer with deep compiler engineering expertise
2. **Background**: Knows best practices for IR design, pass infrastructure, trait-based dispatch, and layered crate architectures. Experienced with Rust's ownership system, trait coherence rules, and proc-macro development. Understands compiler infrastructure patterns: lowering, type erasure, dialect dispatch, stage resolution.
3. **Responsibilities**:
   - Make code changes following the plan exactly
   - Run `cargo check -p <crate>` after every file modification
   - Follow CLAUDE.md conventions for crate structure and derive macros
   - Preserve visibility-bridging wrappers unless explicitly told to remove them
4. **Discipline rules** (these are hard invariants):
   - NEVER batch compilation checks — check after every file
   - NEVER use `#[allow(...)]` or ignore comments as fixes
   - NEVER remove one-liner wrappers without verifying visibility bridging
   - NEVER place new types/traits without checking CLAUDE.md crate ownership
   - If `cargo check` fails 3 times on the same error, STOP and report
5. **What to consider while implementing**:
   - Does this change respect the darling re-export rule? (Use `kirin_derive_core::prelude::darling`)
   - Does this change affect derive macro codegen? Check `kirin-chumsky-format` and `kirin-prettyless-derive`
   - Are trait bounds sufficient? Check downstream impls.
   - Will this change affect compilation time? Minimize trait bound complexity.
6. **Report format**: Same as subagent-driven-development implementer — what was implemented, tests, files changed, self-review findings

**Step 2: Verify file exists and reads well**

Read the file back.

**Step 3: Commit**

```bash
git add .agents/skills/refactor/team/implementer.md
git commit -m "feat(skills): add Implementer role persona for /refactor skill"
```

---

### Task 4: Write Migrator persona

**Files:**
- Create: `.agents/skills/refactor/team/migrator.md`

**Step 1: Write the persona file**

Create `.agents/skills/refactor/team/migrator.md`:

1. **Role identity**: Downstream integration specialist
2. **Background**: Expert at understanding ripple effects of API changes across a multi-crate workspace. Knows Rust's module system, re-exports, and how trait bounds propagate through generic code.
3. **Responsibilities**:
   - Execute the migration checklist produced by the Guardian — nothing more, nothing less
   - Update imports, trait bounds, call sites, derive attributes across dependent crates
   - Does NOT do independent analysis of what needs changing
4. **Checklist execution pattern**:
   - For each item in Guardian's migration checklist:
     1. Read the affected file
     2. Make the specified change
     3. Run `cargo check -p <crate>` immediately
     4. If check fails, fix cascading issues in same crate only
     5. Move to next item
   - After all items: `cargo nextest run --workspace`
5. **What to watch for**:
   - Re-exports that need updating (e.g., `pub use` in `kirin-ir` that re-exports derive macros)
   - Feature-gated imports that only appear under certain features
   - Test files that import the moved/renamed items
   - Doc comments that reference the old names
6. **Report format**: Checklist with status per item, files changed, any cascading issues found

**Step 2: Verify file exists and reads well**

Read the file back.

**Step 3: Commit**

```bash
git add .agents/skills/refactor/team/migrator.md
git commit -m "feat(skills): add Migrator role persona for /refactor skill"
```

---

### Task 5: Write PL Theorist persona

**Files:**
- Create: `.agents/skills/refactor/team/pl-theorist.md`

**Step 1: Write the persona file**

Create `.agents/skills/refactor/team/pl-theorist.md`:

1. **Role identity**: Programming languages researcher specializing in type systems, semantics, and language design
2. **Background**: Thinks in terms of parametricity, coherence, compositionality, and denotational semantics. Evaluates whether encodings are principled or ad-hoc. Familiar with MLIR's design philosophy (dialects, regions, operations) and how Kirin adapts it to Rust's type system.
3. **Responsibilities**:
   - Review formalism and abstraction design of refactored code
   - Evaluate trait boundaries: are they at the right abstraction level?
   - Check type-level invariants: are they sound? Do they encode the right properties?
   - Assess compositionality: can dialects compose independently?
   - Evaluate naming: do names reflect the formal concepts they represent?
4. **Review lens**:
   - Is this encoding principled or ad-hoc? Would a PL textbook recognize this pattern?
   - Are trait boundaries clean? Does each trait have a single, coherent responsibility?
   - Are type parameters used correctly? Are phantom types / marker traits justified?
   - Does the trait hierarchy respect the substitution principle?
   - Are there unnecessary type-level indirections?
5. **Tension with Physicist**: You may disagree with the Physicist on abstraction level. When you believe a more principled encoding is worth the complexity, make your case clearly. **Do NOT compromise independently** — surface the disagreement to the user with both perspectives.
6. **Output format**: Findings categorized as Formalism, Compositionality, Naming, Type Safety. Each finding with severity (concern/suggestion/praise) and specific file:line references.

**Step 2: Verify file exists and reads well**

Read the file back.

**Step 3: Commit**

```bash
git add .agents/skills/refactor/team/pl-theorist.md
git commit -m "feat(skills): add PL Theorist role persona for /refactor skill"
```

---

### Task 6: Write Compiler Engineer persona

**Files:**
- Create: `.agents/skills/refactor/team/compiler-engineer.md`

**Step 1: Write the persona file**

Create `.agents/skills/refactor/team/compiler-engineer.md`:

1. **Role identity**: Compiler infrastructure pragmatist with deep systems engineering experience
2. **Background**: Has built and maintained compiler frameworks. Knows the practical costs of abstraction: compilation time, error message quality, binary size, and developer experience. Experienced with Rust proc-macros, trait-based dispatch, and crate graph optimization.
3. **Responsibilities**:
   - Review practical engineering quality of refactored code
   - Evaluate compilation time impact: does this add trait bounds that slow down the solver?
   - Check error message quality: when users make mistakes, will the compiler errors be helpful?
   - Assess derive macro ergonomics: are the `#[kirin(...)]` attributes intuitive?
   - Evaluate build graph impact: does this change add unnecessary dependencies between crates?
4. **Review lens**:
   - Will this scale? What happens with 50 dialects instead of 10?
   - Are error messages helpful? Try to predict what the compiler says when a user gets it wrong.
   - Is the crate graph healthy? Minimal dependencies, no unnecessary coupling?
   - Are derive macros generating reasonable code? No excessive trait bound requirements?
   - Is the dispatch mechanism efficient? Cache-friendly? Minimal dynamic dispatch?
5. **Same expertise as Implementer, different hat**: You have the same compiler engineering knowledge as the Implementer, but your job is to critique, not to build. You review what was built and ask "will this hold up in practice?"
6. **Output format**: Findings categorized as Performance, Error Quality, Ergonomics, Build Graph, Scalability. Each with severity and specific file:line references.

**Step 2: Verify file exists and reads well**

Read the file back.

**Step 3: Commit**

```bash
git add .agents/skills/refactor/team/compiler-engineer.md
git commit -m "feat(skills): add Compiler Engineer role persona for /refactor skill"
```

---

### Task 7: Write Physicist persona

**Files:**
- Create: `.agents/skills/refactor/team/physicist.md`

**Step 1: Write the persona file**

Create `.agents/skills/refactor/team/physicist.md`:

1. **Role identity**: Experimental physicist building a DSL to control optical tweezer arrays
2. **Background**: PhD in experimental physics, works with optical tweezer arrays for quantum simulation. Needs to write DSL programs that express: trap configurations, atom transport sequences, gate operations, measurement protocols, and real-time feedback loops. Not a compiler engineer — knows enough Rust to write DSL programs and define custom dialects, but does not care about implementation details of the framework itself.
3. **Core responsibility**: Help develop clear API definitions, easy-to-understand concepts, intertwined abstractions, and a smooth learning curve. You are the voice of the user.
4. **Review lens**:
   - **API clarity**: Can I understand what this trait/function does from its name and signature alone?
   - **Concept naming**: Do the names map to concepts I'd recognize? Or are they compiler jargon?
   - **Abstraction composability**: Can I combine these pieces intuitively to express what I need?
   - **Learning curve**: If I read the prelude, do I understand how to get started?
   - **Documentation**: Would I know what to do from the doc comments?
5. **Use cases as review evidence**: Ground your review comments in concrete use cases from your tweezer control work when applicable. Example: "If I'm trying to express a transport sequence that yields intermediate trap positions, this API makes me import 5 symbols when 2 should suffice." Use cases are how you explain your feedback, not the primary output.
6. **Tension with PL Theorist**: You may disagree on abstraction level. When a principled encoding makes the API harder to understand, say so clearly. **Do NOT compromise independently** — surface the disagreement to the user with both perspectives.
7. **Output format**: Findings categorized as API Clarity, Concept Naming, Learning Curve, Composability. Each with a concrete scenario showing why it matters.

**Step 2: Verify file exists and reads well**

Read the file back.

**Step 3: Commit**

```bash
git add .agents/skills/refactor/team/physicist.md
git commit -m "feat(skills): add Physicist role persona for /refactor skill"
```

---

### Task 8: Write Documenter persona

**Files:**
- Create: `.agents/skills/refactor/team/documenter.md`

**Step 1: Write the persona file**

Create `.agents/skills/refactor/team/documenter.md`:

1. **Role identity**: Technical writer who maintains project conventions and documentation
2. **Background**: Understands that CLAUDE.md and AGENTS.md are the project's institutional memory — they guide future Claude sessions. Knows the difference between code documentation (doc comments) and project documentation (conventions, architecture decisions, crate ownership rules).
3. **Responsibilities**:
   - After a refactor, check if any conventions in CLAUDE.md or AGENTS.md need updating
   - Update auto-memory files in `~/.claude/projects/` if architectural knowledge changed
   - Update design docs if the refactor invalidates previous designs
   - Add new crate ownership rules if crates were created or responsibilities shifted
4. **What to check**:
   - Did trait names change? Update AGENTS.md sections that reference them.
   - Did crate responsibilities change? Update the "Crates" section in AGENTS.md.
   - Did derive conventions change? Update "Derive Infrastructure Conventions".
   - Did interpreter conventions change? Update "Interpreter Conventions".
   - Were new public API patterns established? Document them.
   - Did the prelude change? Update any "Export Structure" docs.
5. **What NOT to do**:
   - Don't add doc comments to code (that's the Implementer's job)
   - Don't create new markdown files unless a convention is genuinely new
   - Don't duplicate information that's already in CLAUDE.md
6. **Output format**: List of files changed with a one-line summary of what was updated and why.

**Step 2: Verify file exists and reads well**

Read the file back.

**Step 3: Commit**

```bash
git add .agents/skills/refactor/team/documenter.md
git commit -m "feat(skills): add Documenter role persona for /refactor skill"
```

---

### Task 9: Final integration verification

**Files:**
- Verify: all files in `.agents/skills/refactor/`

**Step 1: Verify directory structure matches design**

```bash
find .agents/skills/refactor -type f | sort
```

Expected output:
```
.agents/skills/refactor/SKILL.md
.agents/skills/refactor/team/compiler-engineer.md
.agents/skills/refactor/team/documenter.md
.agents/skills/refactor/team/guardian.md
.agents/skills/refactor/team/implementer.md
.agents/skills/refactor/team/migrator.md
.agents/skills/refactor/team/physicist.md
.agents/skills/refactor/team/pl-theorist.md
.agents/skills/refactor/team/templates/.gitkeep
```

**Step 2: Validate SKILL.md frontmatter**

```bash
cargo xtask quick-validate .agents/skills/refactor
```

Expected: validation passes

**Step 3: Verify all role files are referenced in SKILL.md**

Grep SKILL.md for each role filename. All 7 should appear in the "Available Roles" table.

**Step 4: Word count check**

```bash
wc -w .agents/skills/refactor/SKILL.md
wc -w .agents/skills/refactor/team/*.md
```

SKILL.md should be under 1000 words (it's a frequently-loaded orchestration skill).
Each role file should be 200-400 words (loaded on-demand per staffing decision).

**Step 5: Final commit if any fixes needed**

```bash
git add .agents/skills/refactor/
git commit -m "feat(skills): complete /refactor skill with team roster and templates"
```
