# Kirin RFCs

An RFC (Request for Comments) is a design proposal for meaningful changes to Kirin:

- public API changes
- IR/dialect/function-model changes
- parser/printer syntax changes
- process or architecture decisions with long-term impact

RFCs make decisions explicit, reviewable, and traceable over time.

## RFC 0000 Template

The canonical template is **RFC 0000**: [`0000-template.md`](./0000-template.md).

`cargo xtask new-rfc ...` renders that Tera template and writes a new RFC file under `rfc/` as:

- `<id>-<title>.md` (example: `0001-the-zen-of-kirin.md`)

## Workflow

1. Create an RFC draft:
   `cargo xtask new-rfc "<title>"`
   If an RFC with the same generated title slug already exists, the command errors instead of creating a duplicate.
   To update the existing RFC timestamp for that title, use:
   `cargo xtask new-rfc "<title>" --update`
   To update both timestamp and status on an existing RFC, use:
   `cargo xtask new-rfc "<title>" --update --status Implemented`
2. Optionally set metadata at creation time:
   `cargo xtask new-rfc "<title>" --status Review --agent codex --author alice --tracking-issue <issue>`
3. Fill in the body placeholders in the generated markdown file.
4. Keep metadata up to date as discussion and implementation progress.

If `--author` is not provided, `xtask` tries to read Git author info (`user.name`/`user.email`).
If Git lookup fails, it sets `authors = ["unknown"]`.
If `--agent` is not provided, the `agents` metadata key is omitted from the file.
If `--discussion` or `--tracking-issue` is not provided, those metadata keys are omitted from the file.
If `--supersedes` or `--superseded-by` is not provided, those metadata keys are omitted from the file.
With `--update`, provided `--author` and `--agent` values are appended (deduplicated) to existing `authors`/`agents` lists.

## Metadata Fields

The metadata block at the top of each RFC is TOML.

| Field | Meaning |
| --- | --- |
| `rfc` | RFC identifier, zero-padded (`0001`, `0042`, ...). |
| `title` | Human-readable RFC title. |
| `status` | Current lifecycle state (`Draft`, `Review`, `Accepted`, `Rejected`, `Implemented`, `Superseded`, `Withdrawn`). |
| `agents` | Optional: list of agent names that authored the RFC (for example `["codex"]`). Only present when provided. |
| `authors` | Author list (names or handles). |
| `created` | RFC creation timestamp (UTC RFC3339). |
| `last_updated` | Last substantive update timestamp (UTC RFC3339). |
| `discussion` | Optional: link (or pointer text) to the review discussion thread. Only present when provided. |
| `tracking_issue` | Optional: link or identifier for implementation tracking. Only present when provided. |
| `supersedes` | Optional: list of RFC IDs this RFC replaces. Only present when provided. |
| `superseded_by` | Optional: RFC ID that replaces this RFC. Only present when provided. |
