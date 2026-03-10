# Chumsky Trait System Refactor — Options Comparison

## Problem

Block/Region-containing dialect types cannot use `#[wraps]` in language enums due to HRTB
(`for<'src>`) trait solver overflow (E0275). This forces dialect authors to inline these
variants, breaking derive composability and requiring manual `Interpretable`/`SSACFGRegion`
impls.

## Options At a Glance

| | A: Monomorphic | B: Box::leak | C: Type-Erased | D: Minimal | E: Staged |
|---|---|---|---|---|---|
| **Fixes statement parsing** | Yes | Yes | Yes | Yes | Phase 1 |
| **Fixes pipeline parsing** | Yes | Partial | Yes | No | Phase 2 |
| **Full `#[wraps]` composability** | Yes | No | Yes | No | Phase 2 |
| **New derive macros** | `ParseDispatch` | None | None | None | Phase 2 |
| **Risk** | Medium | Low | Very High | Low | Low per phase |
| **Effort** | Large | Small | Very Large | Small-Medium | Large (total) |
| **Simplifies trait system** | Yes | No | Somewhat | Somewhat | Yes (final) |
| **Memory overhead** | None | Leaks input | Box per AST node | None | None |
| **Eliminates HRTB** | Fully | Statement only | Fully | Statement only | Fully (Phase 2) |
| **Custom parser hooks** | Yes (Change 4) | No | Harder | No | Phase 3 |

## Detailed Comparison

### Option A: Monomorphic Dispatch + Single Lifetime
**[option-a-monomorphic-dispatch.md](option-a-monomorphic-dispatch.md)**

The comprehensive fix. Four changes that eliminate HRTB entirely, simplify the trait system,
and enable full composability.

- **Best for:** Getting the architecture right long-term
- **Worst for:** Teams that need a quick fix now

### Option B: Box::leak Lifetime Extension
**[option-b-box-leak.md](option-b-box-leak.md)**

Quick band-aid that extends input lifetime to `'static` using `Box::leak`.

- **Best for:** Quick unblocking if only statement parsing is needed
- **Worst for:** Pipeline parsing, production use, memory-sensitive environments

### Option C: Type-Erased Emit
**[option-c-type-erased-emit.md](option-c-type-erased-emit.md)**

Break the HRTB chain by type-erasing `EmitIR` bounds via `Box<dyn DynEmit>`.

- **Best for:** Theoretical elegance
- **Worst for:** Practical implementation — massive risk, performance overhead, loss of type safety

### Option D: Minimal Single Lifetime
**[option-d-minimal-single-lifetime.md](option-d-minimal-single-lifetime.md)**

Apply only the single-lifetime collapse + `ParseStatementText<'t>`, skip monomorphic dispatch.

- **Best for:** Quick cleanup that partially helps
- **Worst for:** Pipeline parsing (doesn't fix it)

### Option E: Staged Approach (D → A)
**[option-e-staged-approach.md](option-e-staged-approach.md)**

Implement D first, then add monomorphic dispatch and custom parser hooks incrementally.

- **Best for:** Risk-averse development with incremental progress
- **Worst for:** Those who want the full fix in one shot

## Recommendation

**Option A** if you want the right answer and are willing to invest in a large refactor.

**Option E** if you want the right answer but want to de-risk the implementation path.

Options B and D are partial fixes that leave the pipeline HRTB unsolved — acceptable only
if pipeline parsing with `#[wraps]` Block/Region types is not a near-term requirement.

Option C is architecturally interesting but the implementation cost and risk far outweigh
the benefits compared to Option A.

### My Pick: **Option A** (or equivalently, **Option E** skipping straight to Phase 2)

The single lifetime collapse (Change 1) and monomorphic dispatch (Change 3) are the two
essential changes. They can be done together in one PR. `ParseStatementText<'t>` (Change 2)
falls out naturally. Custom parser hooks (Change 4) can follow later.

The combined effort of Changes 1-3 is less than it might seem — most of the work is
mechanical signature updates (Change 1) plus one new derive macro (Change 3). The pipeline
parsing logic in `Pipeline<S>` actually gets simpler because it delegates to `ParseDispatch`
instead of implementing everything inline.
