# RFC Implementation Checklist

Use this checklist after coding and before marking an RFC as implemented.

## 1. Requirement Coverage

- List each normative RFC requirement and link corresponding code paths.
- Confirm tests cover each behavior promised by the RFC.
- Confirm migration or compatibility notes in the RFC are reflected in code and docs.

## 2. Behavior Match

- Verify implemented behavior matches RFC wording, not only inferred intent.
- Identify any intentional deviations and document them explicitly.
- Resolve unresolved mismatches before setting RFC status to `Implemented`.

## 3. Validation

- Run relevant crate-level tests.
- Run broader workspace tests when cross-crate behavior changed.
- Run formatting and snapshot checks when affected by the RFC.

## 4. RFC Metadata Update

- Update RFC metadata with xtask, not manual metadata edits:
  - `cargo xtask new-rfc "<rfc-title>" --update --status Implemented`
- Include `--agent codex` when acting as Codex.
- Confirm `status = "Implemented"` and `last_updated` changed in the RFC file.
