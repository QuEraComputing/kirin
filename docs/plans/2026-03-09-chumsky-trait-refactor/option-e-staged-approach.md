# Option E: Staged Approach (D → A incrementally)

## Approach

Implement Option D first (single lifetime + `ParseStatementText<'t>`), then incrementally
add Option A's remaining changes (monomorphic dispatch, custom parser hooks) in follow-up PRs.

This reduces risk by breaking the large refactor into independently shippable milestones.

### Phase 1: Single Lifetime (Option D)

**Scope:** Change 1 + Change 2 from Option A.

- Collapse `HasParser<'tokens, 'src>` → `HasParser<'t>`
- `ParseStatementText<'t, L>` with lifetime parameter
- Update all impls, derives, bounds

**Delivers:**
- Simplified codegen (no dual-bound maintenance)
- Statement-level parsing fixed (no HRTB)
- Cleaner mental model

**Toy-lang status:** Statement parsing works with `#[wraps]`. Pipeline parsing still requires
inlined variants.

### Phase 2: Monomorphic Dispatch

**Scope:** Change 3 from Option A.

- `#[derive(ParseDispatch)]` on stage enums
- Monomorphic match arms for pipeline parsing
- Remove/simplify `SupportsStageDispatchMut`

**Delivers:**
- Pipeline parsing fixed (no HRTB)
- Full `#[wraps]` composability
- Toy-lang can use all dialect types via `#[wraps]`

### Phase 3: Custom Parser Hooks (optional)

**Scope:** Change 4 from Option A.

- `#[chumsky(parser = expr)]` attribute
- Derive generates delegation to custom combinator

**Delivers:**
- Advanced use cases without manual `HasParser` impls
- Better DX for complex dialect types

## Milestone Boundaries

Each phase is independently shippable and testable:

| Phase | Shippable? | Tests Pass? | Breaking? |
|-------|-----------|-------------|-----------|
| 1     | Yes       | Yes (with updates) | Yes — trait signatures change |
| 2     | Yes       | Yes         | Yes — new derive required on stages |
| 3     | Yes       | Yes         | No — additive only |

Since Phases 1 and 2 are both breaking, they could be combined into a single release.
But developing them as separate PRs reduces review complexity.

## Pros

- **Reduced risk** — each phase is independently testable and revertable
- **Incremental value** — Phase 1 alone improves the codebase
- **Flexible schedule** — Phase 3 can be deferred indefinitely
- **Easier review** — smaller PRs are easier to review than one massive refactor
- **Same end state as Option A** — no compromises on the final architecture

## Cons

- **Two breaking changes** — Phases 1 and 2 both break API, annoying if shipped separately
- **Interim state is partial** — between Phase 1 and 2, pipeline parsing still has HRTB
- **More total effort** — integration testing at each phase boundary adds overhead
- **Temptation to stop at Phase 1** — if Phase 1 is "good enough", Phase 2 might never happen
