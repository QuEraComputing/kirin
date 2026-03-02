# Review Follow-up Implementation Plans

**Parent review**: `../2026-03-01-codebase-design-review.md`

These plans are orthogonal workstreams derived from the 2026-03-01 codebase design review. Each can be worked in a separate git worktree without conflicts.

## Workstreams

| # | Plan | Primary Crates | Priority Items |
|---|------|---------------|----------------|
| 1 | [IR Core](./01-ir-core.md) | kirin-ir | P0 gc docs, P1 PhantomData removal, P1 function_by_name, P3 cleanup |
| 2 | [Interpreter Dispatch](./02-interpreter-dispatch.md) | kirin-interpreter, kirin-derive-interpreter | P0 call_handler panic, P1 cached/non-cached unification, P1 stage helper |
| 3 | [Parser Two-Pass & Ergonomics](./03-parser-ergonomics.md) | kirin-chumsky, kirin-chumsky-derive, kirin-chumsky-format | P0 forward-ref fix, P1 HasParser collapse, P2 ParseDialect |
| 4 | [Derive Infrastructure](./04-derive-infra.md) | kirin-derive-core, kirin-derive-dialect, kirin-derive | P0 lattice validation, P1 PhantomData auto-inject, P1 crate merge |
| 5 | [Pretty Printer](./05-pretty-printer.md) | kirin-prettyless, kirin-prettyless-derive | P1 builder pattern, P2 trait consolidation, P3 dead code |
| 6 | [Dialect Cleanup](./06-dialect-cleanup.md) | kirin-cf, kirin-scf, kirin-arith, kirin-function | P0 div/rem panic, P1 Return dedup, P2 docs |

## Dependency Notes

- Plans 1 and 2 have a soft dependency: Plan 1's `Pipeline::function_by_name()` simplifies Plan 6's dialect interpret impls. Work can proceed in parallel — Plan 6 adapts to whichever lands first.
- Plan 4 (PhantomData auto-inject) simplifies dialect code in Plan 6, but Plan 6 can proceed without it.
- All other plans are fully independent.

---

## Agent Architecture

### Design Principles

1. **Autonomous execution**: Agents make best-judgment decisions without asking the user. No interactive questions during implementation.
2. **Deferred uncertainty**: When an agent is unsure, it records the decision it made AND the alternatives it considered in a structured uncertainty log. The lead collects these into a final report.
3. **Quality-gated merges**: No plan merges without review from a specialized reviewer agent.
4. **Deferred dependencies**: Plan 6 Phase 3 is deferred until Plans 1+2 are approved.

### Agent Roster (10 agents)

#### Tier 1: Coordinator

| Agent | Role | Writes Code? |
|-------|------|-------------|
| `lead` | Orchestrate tasks, dispatch work, collect uncertainty reports, produce final report | No |

#### Tier 2: Implementers (6 agents)

Each works in an isolated git worktree. Each reads their plan doc as their specification.

| Agent | Plan | Worktree Branch | Subagent Type |
|-------|------|----------------|---------------|
| `ir-impl` | [Plan 1](./01-ir-core.md) | `review/01-ir-core` | `general-purpose` |
| `interp-impl` | [Plan 2](./02-interpreter-dispatch.md) | `review/02-interpreter` | `general-purpose` |
| `parser-impl` | [Plan 3](./03-parser-ergonomics.md) | `review/03-parser` | `general-purpose` |
| `derive-impl` | [Plan 4](./04-derive-infra.md) | `review/04-derive` | `general-purpose` |
| `printer-impl` | [Plan 5](./05-pretty-printer.md) | `review/05-printer` | `general-purpose` |
| `dialect-impl` | [Plan 6](./06-dialect-cleanup.md) Phases 1-2-4-5 | `review/06-dialects` | `general-purpose` |

#### Tier 3: Specialized Reviewers (3 agents)

Reviewers do NOT write code. They review implementer output and either approve or send change requests with specific feedback.

| Agent | Reviews | Domain Expertise | Subagent Type |
|-------|---------|-----------------|---------------|
| `review-core` | Plans 1 + 2 | IR internals, interpreter dispatch, lifetime patterns | `feature-dev:code-reviewer` |
| `review-frontend` | Plans 3 + 4 | Parser traits, derive macros, HRTB bounds, code generation | `feature-dev:code-reviewer` |
| `review-surface` | Plans 5 + 6 | Pretty printing, dialect conventions, downstream ergonomics | `feature-dev:code-reviewer` |

### Communication Protocol

```
                      ┌──────────┐
                      │   lead   │
                      └────┬─────┘
             ┌─────────┬───┴──┬──────────┐
             ▼         ▼      ▼          ▼
      ┌─────────────────────────────────────┐
      │          Implementers (6)           │
      │  ir  interp  parser  derive         │
      │  printer  dialect                   │
      └──┬──────────────────────────┬───────┘
         │ "unsure, need advice"    │ "phase done"
         ▼                          ▼
      ┌─────────────────────────────────────┐
      │       Specialized Reviewers (3)     │
      │  review-core  review-frontend       │
      │  review-surface                     │
      └──┬──────────────────────────┬───────┘
         │ advice / approve / reject│ "unsure, escalating"
         ▼                          ▼
             ┌──────────┐
             │   lead   │  (decides, records, triggers)
             └──────────┘
```

#### Message Types

| From | To | When | Content |
|------|----|------|---------|
| **Dispatch** |
| `lead` → implementer | Start of work | "Begin Plan N. Your plan doc is at `docs/plans/.../0N-*.md`. Work in worktree `review/0N-*`." |
| `lead` → `dialect-impl` | After Plans 1+2 approved | "Plans 1+2 merged. Begin Plan 6 Phase 3." |
| **Mid-implementation consultation** |
| implementer → reviewer | Unsure about a design decision | "CONSULT: [uncertainty block]. I'm leaning toward X. What do you think?" Implementer **pauses current item** and waits for advice. |
| reviewer → implementer | Reviewer is confident | "ADVICE: Do X because Y." or "ADVICE: Your instinct is right, proceed with X." |
| reviewer → `lead` | Reviewer is also unsure | "ESCALATE: [uncertainty block + reviewer's analysis]. We see options A and B, tradeoffs are [...]." |
| `lead` → reviewer → implementer | Lead decides | "DECISION: Go with A because [reasoning]." Lead records the decision in uncertainty log. |
| **Phase completion review** |
| implementer → `lead` | Phase complete | "Phase N complete. Uncertainty log: [list]. Ready for review." |
| `lead` → reviewer | After implementer signals done | "Review Plan N in worktree `review/0N-*`. Focus on [domain-specific concerns]." |
| reviewer → implementer | During review | Change requests: "In `file.rs:42`, this should be X because Y." |
| implementer → reviewer | After addressing feedback | "Changes addressed. [summary]." |
| reviewer → `lead` | Review complete | "Plan N approved." OR "Plan N needs revision: [blocking issues]." |
| **Final** |
| `lead` → user | All work complete | Consolidated report with all uncertainty decisions. |

### Escalation Protocol

When an implementer hits uncertainty during implementation:

```
implementer encounters uncertainty
  │
  ├─ Can I resolve this with >80% confidence from the plan doc + codebase?
  │   YES → choose, record in uncertainty log, continue
  │   NO  ↓
  │
  ├─ PAUSE current item
  ├─ Send CONSULT to assigned reviewer with uncertainty block
  │
  reviewer receives CONSULT
  │
  ├─ Can the reviewer resolve this confidently?
  │   YES → send ADVICE back to implementer
  │         implementer resumes with the advice
  │   NO  ↓
  │
  ├─ Reviewer sends ESCALATE to lead with both perspectives
  │
  lead receives ESCALATE
  │
  ├─ Lead makes a decision based on project-wide context
  ├─ Lead records the decision in the master uncertainty log
  ├─ Lead sends DECISION back through reviewer to implementer
  │
  implementer resumes with the decision
```

**While paused on a CONSULT**, the implementer should continue working on **other independent items** in the same plan if possible. Only truly block if all remaining items depend on the uncertain decision.

### Uncertainty Record Format

Used by all agents when logging uncertain decisions:

```
UNCERTAINTY: [short title]
DECIDED: [what was chosen]
DECIDED-BY: [implementer | reviewer | lead]
ALTERNATIVES: [what was considered]
REASONING: [why this choice]
CONFIDENCE: [high/medium/low]
IMPACT: [what breaks if this is wrong]
```

### Autonomy Rules for Implementers

When an implementer faces a design decision not fully specified in the plan:

1. **If >80% confident**: Choose the most conservative option, record it in uncertainty log, continue working.
2. **If <80% confident**: Pause the current item. Send a CONSULT to the assigned reviewer. Continue working on other independent items in the plan while waiting.
3. **All decisions** — whether self-resolved, reviewer-advised, or lead-decided — get recorded in the uncertainty log with the `DECIDED-BY` field.
4. Low-confidence decisions should be implemented in a way that's easy to change (e.g., behind a method boundary, not inlined across 10 files).

### Autonomy Rules for Reviewers

Reviewers have two modes:

**Advisory mode** (mid-implementation CONSULTs):
- Respond promptly — the implementer is paused.
- If confident, give clear ADVICE with reasoning.
- If unsure, ESCALATE to lead immediately. Do not guess.
- When advising, also consider whether the question reveals a gap in the plan that should be noted for the final report.

**Review mode** (phase-completion reviews):
- Evaluate against: correctness, conventions (AGENTS.md), blast radius, test coverage, simplicity.
- **Approve** if code is correct and well-tested, even with minor style nits.
- **Request changes** only for correctness issues, missing tests, convention violations, or blast-radius concerns.
- **Flag but not block** on uncertainty-logged items — those go to the final report.

### Autonomy Rules for Lead

The lead does NOT write code. The lead:

1. **Dispatches work** to implementers at startup.
2. **Resolves escalations** by making decisions based on project-wide context, AGENTS.md conventions, and the review document.
3. **Records all decisions** in a master uncertainty log.
4. **Triggers deferred work** (Plan 6 Phase 3) when prerequisites are met.
5. **Produces the final report** for the user, consolidating all uncertainty records from all agents.

When making escalated decisions, the lead should favor:
- Consistency with existing codebase patterns over novel approaches
- Reversibility (prefer choices that are easy to undo)
- The option with less blast radius

### Lifecycle

```
Phase 1: Parallel Launch
  lead spawns all 6 implementers + 3 reviewers simultaneously
  implementers begin working on their plans in isolated worktrees
  reviewers are available for both CONSULTs and reviews from the start

Phase 2: Implementation with Advisory Loop
  implementers work through plan phases
  when unsure: CONSULT → reviewer → (optionally) ESCALATE → lead → DECISION
  implementers work on independent items while waiting for advice

Phase 3: Rolling Reviews
  as each implementer completes a phase, lead assigns the reviewer for full review
  reviewer ↔ implementer iterate until approved
  implementer continues to next phase while review is in progress (if phases independent)

Phase 4: Deferred Dependency
  when review-core approves Plans 1 + 2:
    lead messages dialect-impl to begin Plan 6 Phase 3

Phase 5: Final Report
  when all plans are approved:
    lead collects all uncertainty logs (self-resolved + advised + escalated)
    lead collects all reviewer notes
    lead produces a single consolidated report for the user:
      - Summary of what was implemented
      - All uncertain decisions (grouped by confidence: low first)
      - Decisions that need user ratification
      - Any items deferred or descoped during implementation
      - Reviewer observations worth noting
```

### Final Report Structure

The lead produces this report at `docs/plans/2026-03-01-review-followup/REPORT.md`:

```markdown
# Implementation Report

## Summary
- [x] Plan 1: IR Core — approved
- [x] Plan 2: Interpreter Dispatch — approved
- ...

## Decisions Needing Your Input
Items where the team was unsure and chose a default. Grouped by impact.

### High Impact
- **[title]**: We chose X because Y. Alternative was Z. If you prefer Z, [what needs to change].

### Medium Impact
- ...

### Low Impact (FYI only)
- ...

## Reviewer Observations
Notable patterns or concerns raised during review that don't block merge but are worth knowing.

## Deferred Items
Work that was descoped or deferred during implementation, with reasons.
```

---

## Skill Usage Map

Each plan starts with `/using-git-worktrees` and ends with `/verification-before-completion` → `/requesting-code-review` → `/finishing-a-development-branch`. The table below shows the **distinctive** skills per plan.

| Plan | Key Skills | Why |
|------|-----------|-----|
| 1. IR Core | `/brainstorming` (function_by_name, PhantomData), `/test-driven-development`, `/subagent-driven-development` | Phase 1 mechanical fixes can parallelize with Phase 2 design |
| 2. Interpreter Dispatch | `/systematic-debugging` (call_handler panic), `/brainstorming` (dispatch unification, resolve_stage API), `/subagent-driven-development` | Phase 1+4 independent of Phases 2-3 |
| 3. Parser Ergonomics | `/systematic-debugging` (forward-ref), `/brainstorming` (HasParser collapse), `/kirin-rfc-writer` (lifetime change warrants RFC), `/subagent-driven-development` | Highest-risk plan; RFC + careful migration |
| 4. Derive Infrastructure | `/brainstorming` (crate merge), `/test-driven-development` (compile-fail tests), `/subagent-driven-development` | Crate restructuring benefits from parallel agents |
| 5. Pretty Printer | `/brainstorming` (builder pattern, trait consolidation), `/test-driven-development`, `/subagent-driven-development` | Two independent trait consolidations can parallelize |
| 6. Dialect Cleanup | `/systematic-debugging` (div/rem), `/brainstorming` (Return removal, import patterns), `/executing-plans` (adopt Plan 2 helpers), `/subagent-driven-development` | Cross-dialect changes parallelize well |

### Common Workflow Pattern

```
/using-git-worktrees           # Isolate work
  ├─ /brainstorming            # Design before implementation (for non-trivial items)
  ├─ /test-driven-development  # Tests before code (for new APIs and bug fixes)
  ├─ /systematic-debugging     # Root-cause first (for P0 correctness items)
  ├─ /subagent-driven-development  # Parallelize independent sub-tasks
  ├─ /simplify                 # Clean up after each batch of changes
  ├─ /verification-before-completion  # Full test suite before claiming done
  ├─ /requesting-code-review   # Review before merge
  └─ /finishing-a-development-branch  # Merge/PR strategy
```
