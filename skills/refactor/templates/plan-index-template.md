# Plan Index Template

Use this template when generating the root `index.md` for a refactor plan
directory. The index is the orchestration map: the lead agent reads it to
dispatch agents and track progress. It does NOT contain implementation
details — those live in individual plan files.

---

## Template

```markdown
# <Refactor Name> — Plan Index

**Date:** <YYYY-MM-DD>
**Review report:** `docs/review/<root-refactor-name>/report.md`
**Pattern:** <in-place | additive-then-switchover>
**Total findings addressed:** <N accepted out of M total>

---

## Dependency Graph

<Text diagram showing execution order. Low-hanging fruit always runs first.>

```
low-hanging-fruit
       |
    wave-1 (all parallel)
       |
    wave-2 (all parallel, depends on wave-1)
       |
    wave-3 (all parallel, depends on wave-2)
```

## Low-Hanging Fruit

| # | Title | Finding | Crate | Effort |
|---|-------|---------|-------|--------|
| LHF-1 | <title> | <ID> | <crate> | <est.> |
| LHF-2 | ... | ... | ... | ... |

**Plan file:** `low-hanging-fruit.md`

## Wave 1

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| `wave-1/<slug>-plan.md` | <title> | <IDs> | <role> | <crates> |
| ... | ... | ... | ... | ... |

## Wave 2

**Depends on:** Wave 1 complete and merged.

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| ... | ... | ... | ... | ... |

## Wave 3

**Depends on:** Wave 2 complete and merged.

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| ... | ... | ... | ... | ... |

## Agent Assignments

| Agent Name | Role | Wave | Plan File | Files Touched |
|------------|------|------|-----------|---------------|
| <name> | <role> | <wave> | `<path>` | <files> |
| ... | ... | ... | ... | ... |

**File disjointness check:** <Confirm no file overlaps within a wave.
List any shared files and how they are sequenced.>

## Verification Checkpoints

After each wave:
1. `cargo build --workspace`
2. `cargo nextest run --workspace`
3. `cargo test --doc --workspace`
4. `cargo insta test --workspace` (if snapshots exist)

## Excluded Findings

<Findings from the review that were rejected during walkthrough or deferred.>

| Finding | Reason |
|---------|--------|
| <ID> | <reason, e.g., "Won't fix — acceptable complexity"> |
```

---

## Filling guidance

**Root refactor name:** Must match the review directory name. For example,
if the review is at `docs/review/2026-03-21-graph-parsing-refactor/`, the
plan directory is `docs/plans/2026-03-21-graph-parsing-refactor/`.

**Wave grouping rules:**
1. Dependency order — if finding B depends on A's output, A is in an earlier wave
2. File disjointness — findings touching the same files go in the same wave
   and are assigned to the same agent (or sequenced explicitly)
3. Coupled findings — findings linked by the review report's cross-cutting
   themes become a single plan file

**Agent role selection:**
- Builder — creates new crates from scratch (additive-then-switchover pattern)
- Implementer — modifies existing crates (in-place refactors)
- Migrator — updates downstream consumers (imports, Cargo.toml, feature flags)

**File disjointness check:** For each wave, verify that no two agents touch
the same file. If they do, either merge the plan files or explicitly sequence
them (agent A completes before agent B starts on the shared file).
