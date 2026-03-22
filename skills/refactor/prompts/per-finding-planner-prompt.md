# Per-Finding Planner Prompt Template

Use this template when the Lead Planner dispatches a dedicated Planner for
a non-trivial finding or coupled finding group.

**Purpose:** Generate a single self-contained plan file for one finding (or
tightly-coupled finding group). The Planner explores the codebase deeply to
understand the finding's context and produces an actionable plan.

**Dispatch:** `run_in_background: true`. The per-finding Planner is a read-only
research role — it reads the codebase and writes one plan file, but does not
modify source code.

```
Agent (general-purpose):
  description: "Planner for <finding-slug>"
  run_in_background: true
  prompt: |
    You are a Planner for a single refactoring finding. Your job is to
    generate one self-contained plan file. You do NOT modify source code.

    ## Inputs
    - Finding(s): <finding IDs>
    - Finding text:
      <paste the full finding text from the review report — verbatim>
    - Wave: <N>
    - Agent role: <Builder | Implementer | Migrator>
    - Output path: docs/plans/<root-refactor-name>/wave-<N>/<slug>-plan.md
    - Template: skills/refactor/templates/plan-file-template.md

    ## Process

    ### 1. Read the template
    Read skills/refactor/templates/plan-file-template.md in full — it contains
    the structure, required sections, and filling guidance.

    ### 2. Explore the codebase
    Read the files referenced in the finding. Understand:
    - The current code structure and patterns in the affected area
    - Exact file paths and current line numbers (the review may be stale)
    - Downstream consumers that will be affected
    - Existing tests that cover the affected code
    - Relevant AGENTS.md conventions for the affected crate(s)

    Exploration budget:
    - Simple findings (single crate, clear action): 10 file reads + 5 greps
    - Complex findings (multi-crate, design required): 20 file reads + 15 greps

    ### 3. Design work (if needed)
    For findings classified as "design work", explore the design space:
    - Identify alternative approaches
    - Evaluate trade-offs against project conventions
    - Choose the approach that best fits existing patterns
    Document the design rationale in the Guiding Principles section.

    ### 4. Generate the plan file
    Write the plan file at the output path using the template structure.

    Self-containment rule — the plan file MUST include:
    - The original review finding text (not a cross-reference)
    - Relevant AGENTS.md conventions (quoted inline, not by section name)
    - Exact file paths and line numbers (verified by reading the files)
    - All validation commands with expected output
    - Clippy policy: no #[allow(...)], all warnings must be fixed at root cause

    Verify-Before-Build rule — if the plan depends on assumptions about how
    existing code works (API shapes, trait impls, file locations), add explicit
    verification steps in the "Verify Before Implementing" section. The
    implementer runs these first; if any fails, they STOP and report. This
    prevents agents from silently working around stale review findings.

    Regression test rule (P0/P1 findings) — for P0 and P1 severity findings,
    the plan SHOULD include a "Regression Test" section that designs a test
    reproducing the issue BEFORE the fix. Try hard to come up with a test:
    construct an input that triggers the bug, assert on the wrong behavior,
    or use #[should_panic] / debug assertions. If truly infeasible (e.g.,
    UB that only manifests at runtime under specific conditions), explain why
    and describe how the fix will be validated instead. This is preferred,
    not mandatory — but put real effort into it.

    Step granularity — each step should be one action: write test, run test,
    implement change, run tests, fix clippy, commit. Use checkbox syntax
    (`- [ ]`) for tracking. Include expected output for verification steps
    and test runs so the implementer knows what success looks like.

    ### 5. Report
    When done, report:
    - Output file path
    - Files that will be modified (for disjointness verification)
    - Any concerns about feasibility or scope
    - Whether this is a real fix or a workaround (and why)
```

## When to dispatch multiple findings to one Planner

The Lead Planner groups findings into a single Planner dispatch when:
- The review report explicitly links them (cross-cutting themes, shared root cause)
- They touch the same files (must be in the same plan for disjointness)
- Fixing one is prerequisite to fixing the other

In these cases, list all finding IDs and paste all finding texts in the prompt.
The Planner produces a single plan file that addresses all of them.
