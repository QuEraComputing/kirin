---
name: kirin-rfc-implementer
description: Implement an existing Kirin RFC end-to-end and verify the delivered code matches the RFC intent. Use when executing an approved RFC, translating it into code and tests, validating behavior, checking implementation-to-RFC alignment, and updating the RFC metadata status to Implemented using `cargo xtask new-rfc --update`.
---

# Kirin RFC Implementer

## Overview

Implement RFC-scoped changes in the Kirin workspace with a traceable workflow from RFC requirements to code, tests, and final RFC status updates.

## Workflow

1. Pin the target RFC and implementation boundary.
   - Identify the RFC file and exact title.
   - Separate required scope from explicit non-goals before editing code.
2. Translate the RFC into an execution checklist.
   - Extract concrete requirements (APIs, behavior, migration, validation expectations).
   - Map each requirement to crates/modules/tests that will change.
3. Implement incrementally.
   - Apply small, reviewable commits and keep changes scoped to RFC goals.
   - Follow repository rules in `AGENTS.md` (module split guidance, import discipline, shared test utilities).
4. Validate behavior.
   - Run targeted checks first (`cargo test -p <crate> ...`), then broader checks as needed.
   - Run `cargo fmt --all` when formatting-sensitive files changed.
5. Double-check RFC alignment after implementation.
   - Compare each RFC requirement against merged code paths and tests.
   - Record concrete evidence using file paths and symbols.
   - If behavior diverges from RFC text, either fix the implementation or explicitly call out the gap before completion.
6. Update RFC metadata status using xtask.
   - Run `cargo xtask new-rfc "<rfc-title>" --update --status Implemented`.
   - If acting as an agent, append agent metadata with `--agent codex`.
   - Do not hand-edit the RFC metadata block.
7. Provide completion output.
   - Summarize implemented requirements, validation commands, and any remaining follow-up items.

## Alignment Gate

Use `references/rfc-implementation-checklist.md` before declaring completion.

If any required checklist item fails, do not mark the RFC implemented until the gap is resolved or explicitly accepted by the user.
