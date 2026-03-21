# Cross-Review Prompt Template

After all initial reviews for a crate are complete, each reviewer reads the other reviewers' reports.

```
You are the <Role> Reviewer. You have completed your initial review of <crate>.

Now read the other reviewers' reports:
- [path to reviewer A report]
- [path to reviewer B report]

For each finding in the other reports, assess:

1. **Agree / False Positive / Severity Adjust**
   - agree: Finding is genuine. No comment needed unless you have additional evidence.
   - false-positive: Finding is incorrect or based on a misreading. Explain WHY.
   - severity-adjust: Finding is real but severity is wrong. Suggest new severity with rationale.

2. **Low priority candidates** — Flag findings technically correct but unlikely to matter in practice. Explain why.

3. **Cross-cutting insights** — Findings that become more or less significant when viewed alongside your own.

## Output

### Reviewed Findings
- [agree/false-positive/severity-adjust] <finding reference> — <rationale>

### Low Priority Candidates
- <finding reference> — <why it's low priority>

### Cross-Cutting Insights
- <insight connecting findings across reviewers>
```

## Special Rule: Code Quality Reviewer

Append this to the Code Quality reviewer's cross-review prompt:

```
## Special Requirement (Code Quality Only)

Pay special attention to the Formalism reviewer's report. For each
alternative formalism or abstraction they propose:
1. Identify concrete code locations where adopting that abstraction would
   eliminate existing duplication you found in your initial review
2. Add supplementary findings under a `### Formalism-Informed Duplication` section
3. For each, estimate lines saved and complexity of the refactor
```
