+++
rfc = "{{ rfc_id }}"
title = "{{ title_toml }}"
status = "{{ status }}"
{% if agents -%}
agents = [{% for agent in agents %}"{{ agent }}"{% if not loop.last %}, {% endif %}{% endfor %}]
{% endif -%}
authors = [{% for author in authors %}"{{ author }}"{% if not loop.last %}, {% endif %}{% endfor %}]
created = "{{ timestamp }}"
last_updated = "{{ timestamp }}"
{% if discussion -%}
discussion = "{{ discussion }}"
{% endif -%}
{% if tracking_issue -%}
tracking_issue = "{{ tracking_issue }}"
{% endif -%}
{% if dependencies -%}
dependencies = [{% for rfc in dependencies %}"{{ rfc }}"{% if not loop.last %}, {% endif %}{% endfor %}]
{% endif -%}
{% if supersedes -%}
supersedes = [{% for rfc in supersedes %}"{{ rfc }}"{% if not loop.last %}, {% endif %}{% endfor %}]
{% endif -%}
{% if superseded_by -%}
superseded_by = "{{ superseded_by }}"
{% endif -%}
+++

# RFC {{ rfc_id }}: {{ title }}

Replace all bracketed placeholders and remove instructional lines that do not apply.

## Summary

[2-5 sentences: what changes, why, and expected outcome]

## Motivation

- Problem: [current pain or limitation]
- Why now: [new requirement, bug pattern, or scaling issue]
- Stakeholders: [crates, maintainers, users, tooling]

## Goals

- [Goal 1]
- [Goal 2]

## Non-goals

- [Explicitly out of scope]

## Guide-level Explanation

[Explain the user-facing behavior and usage model in plain language.]

## Reference-level Explanation

### API and syntax changes

- [Public API, derive attributes, text syntax, or CLI behavior]
- [Example signatures/snippets]

### Semantics and invariants

- [Function/statement/region/signature model changes]
- [Invariants and ownership/lifetime implications]

### Crate impact matrix

This subsection is illustrative. Keep only impacted crates.

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-ir` | [summary] | [targets] |
| `kirin-chumsky` | [summary] | [targets] |
| `kirin-prettyless` | [summary] | [targets] |

## Drawbacks

- [Cost, complexity, compatibility risk, or maintenance burden]

## Rationale and Alternatives

This section structure is illustrative. Keep at least one concrete alternative, and rename subsections when needed.

### Proposed approach rationale

- [Why this design is preferable]

### Alternative A

- Description: [approach]
- Pros: [list]
- Cons: [list]
- Reason not chosen: [short explanation]

### Alternative B

- Description: [approach]
- Pros: [list]
- Cons: [list]
- Reason not chosen: [short explanation]

## Prior Art

- [Related Rust RFC / PEP / language design]
- [What we borrow or intentionally avoid]

## Backward Compatibility and Migration

- Breaking changes: [none / list]
- Migration steps: [step-by-step]
- Compatibility strategy: [feature flags, dual parser support, deprecations]

## How to Teach This

- [How maintainers/users should learn and adopt this]
- [Docs/examples that need updates]

## Reference Implementation Plan

This section is optional for very small RFCs; include when sequencing matters.

1. [Implementation slice 1]
2. [Implementation slice 2]
3. [Implementation slice 3]

### Acceptance Criteria

- [ ] [Measurable criterion 1]
- [ ] [Measurable criterion 2]

### Tracking Plan

- Tracking issue: [link]
- Implementation PRs: [links]
- Follow-up tasks: [links]

## Unresolved Questions

- [Question with owner/decision needed]

## Future Possibilities

- [Out-of-scope follow-on direction]

## Revision Log

| Date | Change |
| --- | --- |
| {{ timestamp }} | RFC created from template |
