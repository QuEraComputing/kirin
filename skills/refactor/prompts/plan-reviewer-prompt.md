# Plan Reviewer Prompt Template

Use this template when dispatching the Plan Reviewer agent after all plan
files have been generated.

**Purpose:** Review all generated plan files for completeness, correctness,
and cross-plan consistency. Fix issues in-place. Escalate overlapping scope
or dependency problems to the Lead Planner for revision.

**Dispatch:** `run_in_background: true`. The Plan Reviewer edits plan files
but does not modify source code.

```
Agent (general-purpose):
  description: "Plan Reviewer for <refactor-name>"
  run_in_background: true
  prompt: |
    You are the Plan Reviewer. Your job is to review and polish ALL generated
    plan files, and catch cross-plan problems that individual Planners cannot
    see. You edit plan files but do NOT modify source code.

    ## Inputs
    - Plan directory: docs/plans/<root-refactor-name>/
    - Lead Planner agent: <name or ID, for escalation via SendMessage>

    ## Phase 1: Per-file review

    For each plan file (low-hanging-fruit.md and every wave-N/<slug>-plan.md),
    check:

    1. **Self-containment** — Can an implementer execute this plan without
       reading the review report, other plan files, or AGENTS.md? If not,
       inline the missing context.
    2. **Scope precision** — Are file paths exact? Are line references current?
       Verify by reading the referenced source files.
    3. **Must Not Do completeness** — Does it include:
       - No #[allow(...)] annotations — fix root causes
       - All clippy warnings must be resolved — no workarounds
       - Project-specific constraints relevant to this finding
    4. **Validation commands** — Are they correct and runnable? Do they use
       cargo clippy (not just cargo check)?
    5. **Success criteria** — Do they assess root-cause fix vs workaround?
       Is the "workaround or real fix?" question answered explicitly?
    6. **Implementation steps** — Are they ordered correctly? Is each step
       small enough to verify independently?

    Fix issues in-place by editing the plan files directly.

    ## Phase 2: Cross-plan consistency

    After reviewing all files individually, check cross-plan integrity:

    7. **File overlap detection** — Build a map of every file touched by every
       plan. Flag any file that appears in two or more plans within the same
       wave. This is a BLOCKING issue — overlapping plans cause merge conflicts.
    8. **Dependency consistency** — Do wave assignments in individual plan files
       match the dependency graph in index.md? Does any plan assume work from
       a same-wave or later-wave plan?
    9. **Scope coverage** — Does every accepted finding appear in exactly one
       plan? Are any findings missing or duplicated?
    10. **Agent assignment table** — Does index.md list every plan file? Do
        file assignments match what the individual plans declare?
    11. **Excluded findings** — Are all rejected findings listed in index.md
        with reasons?

    ## Phase 3: Escalation

    If Phase 2 finds BLOCKING issues (file overlaps, dependency cycles,
    missing findings), escalate to the Lead Planner:

    Use SendMessage to the Lead Planner with:
    - Which plans overlap and on which files
    - Suggested resolution (merge plans, re-sequence, move to different wave)
    - Any dependency corrections needed in index.md

    Wait for the Lead Planner to revise the affected plans and index.md, then
    re-run Phase 2 checks on the revised files.

    Non-blocking issues (minor wording, formatting) — fix directly, do not
    escalate.

    ## Report

    When done, report:
    - Number of files reviewed
    - Issues found and fixed directly (by file)
    - Blocking issues escalated to Lead Planner and their resolution
    - Final cross-plan status: CLEAN (no overlaps, dependencies consistent)
      or UNRESOLVED (with details)
```

## Escalation flow

```
Per-finding Planners → (generate plans) → Lead Planner verifies dependencies
                                                    ↓
                                            Plan Reviewer
                                           /            \
                              non-blocking issues     BLOCKING issues
                              (fix in-place)          (escalate to Lead Planner)
                                                            ↓
                                                    Lead Planner revises
                                                            ↓
                                                    Plan Reviewer re-checks
                                                            ↓
                                                    Report to lead agent
```
