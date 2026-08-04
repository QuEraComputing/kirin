Status: ready-for-agent

# Sound dependency-triggered SSACFG reevaluation

## Problem Statement

Kirin can change the behavior of a valid program when constant propagation and
constant folding analyze a CFG loop whose conditional block reads a
loop-carried value from a dominating block. The loop-carried block argument is
correctly widened to `Unknown`, but a dominated consumer is skipped because its
explicit edge arguments have not changed. The stale constant result is then
written as a hint and materialized by folding, causing a counter that should
reach three to return one. The same generic worklist behavior can make other
forward analyses unsound for legal SSA dependencies.

## Solution

Kirin will preserve ordinary SSA dominance and make its generic SSACFG abstract
interpreter reevaluate reached blocks when an abstract SSA value on which they
depend changes. Dependencies will be discovered through the IR's static use-def
graph, including uses inside nested regions. Dependency generations will extend
the existing successor visitation identity so required reevaluations occur
without discarding path-sensitive edge arguments, duplicate work remains
deduplicated, and existing convergence behavior remains compatible.

## User Stories

1. As a Kirin DSL author, I want optimization passes to preserve loop-carried counter semantics, so that optimized programs behave like their lowered source.
2. As a Kirin DSL author, I want conditional updates inside loops to remain correct, so that runtime conditions do not become compile-time constants accidentally.
3. As a compiler-pass author, I want legal uses of values from dominating blocks to be analyzed soundly, so that I do not need to rewrite valid SSA into a special analysis-only form.
4. As a constant-propagation user, I want an addition with an unknown operand to remain unknown, so that folding never materializes an unjustified value.
5. As a type-inference user, I want the shared forward solver to reevaluate dependent consumers, so that the fix is not limited to constant propagation.
6. As an interpreter extension author, I want dependencies to follow SSA use-def records, so that analysis scheduling matches the dependencies declared by the IR.
7. As an author of region-bearing operations, I want uses inside nested regions to invalidate their enclosing SSACFG block, so that parent-frame reads cannot leave stale results.
8. As a compiler-pass author, I want unreachable blocks to remain unanalyzed, so that dependency scheduling does not invent returns, purity, or side effects from dead control flow.
9. As a compiler maintainer, I want repeated dependency updates to deduplicate at the latest generation, so that soundness does not introduce avoidable worklist churn.
10. As a compiler maintainer, I want existing path-sensitive successor arguments to keep working, so that this correctness fix does not become a broad solver rewrite.
11. As a compiler maintainer, I want visitation changes compared by lattice meaning, so that equivalent abstract states do not trigger redundant work.
12. As a compiler maintainer, I want the existing per-block visitation cutoff retained, so that this change does not alter established convergence behavior.
13. As a regression investigator, I want the earliest invalid analysis result asserted directly, so that future failures localize before constant folding.
14. As a regression investigator, I want the complete fold pipeline asserted through executable behavior, so that a correct analysis result also produces a semantics-preserving optimization.
15. As a contributor, I want the dependency mechanism kept private initially, so that the public API is not generalized before another consumer establishes its shape.
16. As a contributor, I want the architectural reason for dependency-triggered reevaluation recorded, so that a later worklist refactor preserves the soundness contract.
17. As a user with an in-memory method analyzed by an older buggy build, I want the limitation around stale hints documented, so that I know to rebuild or clear that method before reanalysis.

## Implementation Decisions

- Ordinary SSA uses of values from dominating blocks remain valid; lowering will not be changed to thread every varying value through every block argument.
- The generic SSACFG abstract interpreter will use dependency-triggered reevaluation rather than a new liveness analysis.
- Dependencies will be built once per analyzed SSACFG region from static SSA use-def records.
- A use inside a nested region will be attributed to the enclosing block of the active SSACFG region.
- Transfer functions must expose SSA dependencies through the IR rather than reading undeclared SSA values.
- Only blocks reached through normal CFG traversal are eligible for dependency-triggered reevaluation.
- Abstract-value changes will be determined by equivalence in the lattice partial order, not Python identity or equality.
- The existing successor worklist and path-sensitive edge arguments will remain in place.
- Each reached block will have a dependency generation included in visitation identity. A material dependency change advances that generation and requeues the block with its current block arguments.
- Repeated work for the same successor state and dependency generation will deduplicate.
- The existing limit of 128 visited states per block and its silent truncation behavior will remain unchanged.
- The dependency machinery will remain private to the generic solver until another consumer justifies a reusable public abstraction.
- The accepted architectural decision will be recorded in the repository ADR collection.

## Testing Decisions

- Tests will observe behavior through public analysis and optimization entry points rather than calling private dependency helpers.
- A generic SSACFG analysis test will demonstrate that a dominated live-in which widens causes its consumer to be evaluated with the widened value.
- A constant-propagation test will assert that the loop increment's result is `Unknown` after analysis.
- An optimization test will assert that folding the minimal conditional two-iteration loop returns two rather than one.
- A nested-region test will demonstrate that an SSA dependency below a region-bearing statement reevaluates the enclosing SSACFG block.
- An unreachable-block test will demonstrate that a changed dependency does not make a dead consumer executable.
- A deduplication test will use the public custom-analysis extension seam to demonstrate that multiple updates pending for one dependency generation produce one semantic reevaluation.
- Existing constant-propagation, type-inference, and SSACFG interpreter tests provide the prior art for inline IR methods and custom analysis method tables.
- Each vertical slice will be observed failing before its minimal implementation is added.

## Out of Scope

- Changing Python-to-CFG lowering to make every dominated use an explicit block argument.
- Replacing the successor worklist with a new block-state dataflow solver.
- Adding a reusable liveness analysis without another live-in/live-out consumer.
- Changing the existing 128-state convergence cutoff or making it fail loudly.
- Changing constant-hint merge semantics.
- Adding provenance or ownership to constant hints.
- Repairing stale hints already stored on an in-memory method analyzed by the buggy solver.
- Clearing or rebuilding user methods that contain stale hints.

## Further Notes

The standalone reproduction fails deterministically: frontend lowering and
constant propagation alone execute correctly, while constant propagation
followed by constant folding returns one instead of three. Instrumentation
showed the loop-carried input widen from `Value(0)` to `Unknown`; the dominated
addition block retained `Value(1)` because it was visited only under an unchanged
explicit boolean edge argument. Recomputing the addition manually returned
`Unknown`, confirming that scheduling—not arithmetic or lattice joining—is the
root cause.

Constant hints currently behave as independently valid lower bounds and are
combined with new analysis information using lattice meet. That separate
contract is why a previously invalid hint cannot self-heal and why hint
provenance is intentionally deferred.
