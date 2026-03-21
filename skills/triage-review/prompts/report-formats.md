# Report Format Templates

## Per-Crate Final Report

```markdown
# <crate> — Final Review Report

Consolidated from <N> reviewer perspectives: <list>.

---

## High Priority (P0-P1)

### 1. <Finding title>
**Severity:** P1 | **Confidence:** confirmed | **Source:** <Reviewer>
<Clear explanation with code reference>
**Suggested action:** <specific next step>

---

## Medium Priority (P2)
### N. <Finding title>
...

## Low Priority (P3)
...

## Strengths
- ...

## Filtered Findings
<details>
<summary>N findings filtered</summary>
- <finding> — filtered because: <reason>
</details>
```

## Full Workspace Report

```markdown
# <Scope> Review — YYYY-MM-DD

**Scope:** <description>
**Reviewers:** <list>
**Per-crate reports:** `<review-dir>/<datetime>/<crate>/final-report.md`

---

## Executive Summary

<2-3 sentences on overall health and key findings>

| Severity | Accepted | Won't Fix | Total |
|----------|----------|-----------|-------|
| P0-P3 rows... |

---

## P1 Findings (All Crates)

### P1-1. <Finding title>
**Crate:** <crate> | **File:** `<file:line>`
<Clear explanation>
**Action:** <specific follow-up>
**References:** [<external link if applicable>]

---

## P2 Findings
...

## P3 Findings
...

## Cross-Cutting Themes

### 1. <Theme name> (<N> reviewers, <N> crates)
<explanation with code examples>

---

## Architectural Strengths
1. ...

---

## Follow-Up Actions (Priority Order)

### Quick Wins (< 30 min each)
1. <action> — <crate> — <file:line>

### Moderate Effort (1-3 hours each)
...

### Design Work (half-day+)
...

### Documentation
...

---

## Filtered Findings

<details>
<summary>N findings filtered across all crates</summary>
- <finding> — filtered because: <reason>
</details>
```
