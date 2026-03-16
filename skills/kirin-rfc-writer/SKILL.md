---
name: kirin-rfc-writer
description: Draft, revise, and review Request for Comments (RFC) design documents for the Kirin framework. Use when proposing or changing dialects, IR and function model behavior, parser or pretty-printer interfaces, crate boundaries, public APIs, or migration strategy, and when turning implementation ideas into structured RFC documents under `rfc/`.
---

# Kirin RFC Writer

## Overview

Create RFC-style design documents that match Kirin conventions, stay grounded in concrete code locations, and follow the repository RFC flow based on `cargo xtask new-rfc`.

## Workflow

1. Collect scope and constraints.
   - Confirm the target change, motivation, and non-goals.
   - Identify affected crates and modules before drafting.
   - Read relevant files in `design/` and impacted crate code first.
2. Create the RFC file first.
   - Run `cargo xtask new-rfc "<title>"`.
   - Optionally pass metadata overrides, for example: `--status Review --agent codex --author alice --discussion <url> --tracking-issue <id> --supersedes <rfc> --superseded-by <rfc>`.
   - If you are an agent creating the RFC, always pass `--agent <agent-name>` using your own identity (for example, Codex must use `--agent codex`).
   - Use `--update` to refresh `last_updated` on an existing RFC with the same title slug before editing.
   - Use `--update --status <Status>` to update both `last_updated` and `status` on an existing RFC with the same title slug.
   - When using `--update`, any provided `--author` or `--agent` values are appended to existing `authors` and `agents` metadata lists instead of replacing them.
   - For RFC creation (without `--update`): if `--author` is omitted, `xtask` auto-detects Git author info; if detection fails, it uses `unknown`.
   - For RFC updates (`--update`): if `--author` is omitted, the `authors` list is not changed.
   - Run from any subdirectory; `xtask` resolves the Kirin root and writes to `rfc/<id>-<title>.md`.
   - The command renders the Tera template `rfc/0000-template.md` and fills generated metadata values (`rfc`, `title`, `status`, `authors`, `created`, `last_updated`, and `agents` when `--agent` is passed).
3. Fill placeholders in the generated RFC file.
   - Replace every bracketed placeholder with concrete content.
   - Do not directly edit the TOML metadata block by hand.
   - Set metadata with `cargo xtask new-rfc` options at creation time.
   - Update metadata with `cargo xtask new-rfc "<title>" --update` and supported flags (`--status`, `--author`, `--agent`).
   - Remove template-only guidance text that is not part of the final RFC.
   - Link proposed behavior to specific crates, modules, and public types.
4. Handle illustrative and optional sections intentionally.
   - `Alternative A/B` headings are illustrative; rename or restructure as needed.
   - `Crate impact matrix` entries are illustrative; keep only affected crates.
   - `Reference Implementation Plan` can be omitted for tiny RFCs.
5. Evaluate alternatives and risk.
   - Describe at least two alternatives with clear trade-offs.
   - Cover compatibility, migration, and failure modes.
   - Define validation work (tests, snapshots, roundtrip checks, benchmarks).
6. Close with actionable outcomes.
   - End with explicit open questions and decision points.
   - List implementation slices in dependency order.

## Kirin-specific requirements

- Keep terminology consistent with the codebase: `Dialect`, `StageInfo`, `Function`, `StagedFunction`, `SpecializedFunction`, `SignatureSemantics`, `HasParser`, `PrettyPrint`.
- If a dialect is standalone and contains only one statement/op, always model it as a `struct` (not an `enum`).
- For dialect-creation implementation guidance, prefer defining each statement/op as a separate `struct`, then define the dialect as a wrapper `enum` over those structs to maximize reuse.
- If a statement group is expected to always be imported and used together (for example, arithmetic operations), prefer a single `enum` with one variant per statement/op instead of separate structs.
- Match existing design-doc tone in `design/*.md`: concise explanation first, then examples and code locations.
- Prefer concrete file references over abstract descriptions.
- For syntax work, describe parser/printer roundtrip expectations (`print -> parse -> print`) when relevant.
- For testing plans, name exact crates and test targets that should change.
- Use RFC status values consistently: `Draft`, `Review`, `Accepted`, `Rejected`, `Implemented`, `Superseded`, `Withdrawn`.

## Quality gate

- Run the checklist in `references/rfc-checklist.md` before finalizing.
- If any checklist item is not satisfied, call it out explicitly in the RFC.
