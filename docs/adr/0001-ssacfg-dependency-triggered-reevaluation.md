---
status: accepted
---

# Reevaluate SSACFG blocks through abstract-value dependencies

Kirin's generic SSACFG abstract interpreter will revisit a reached block when
an abstract SSA value on which that block depends changes. This preserves
soundness for ordinary SSA uses of values from dominating blocks, including
uses inside nested regions. Dependencies are discovered from the IR's static
SSA use-def graph, with nested uses attributed to their enclosing SSACFG block;
transfer functions must not hide SSA dependencies from the IR. A separate
liveness analysis should be introduced only when a consumer needs live-in or
live-out queries, because using liveness snapshots as worklist keys would couple
the forward solver to an otherwise unnecessary backward analysis. The existing
successor worklist and its path-sensitive edge arguments remain intact;
dependency invalidation extends that mechanism instead of replacing it with a
new block-state solver. Dependency changes requeue only blocks already proven
reachable by normal CFG traversal, with reachability tracked separately from
the visited-state cache so unreachable code is not analyzed speculatively.
Each reached block has a dependency generation that advances when one of its
abstract SSA dependencies changes; the generation is part of visitation
identity, preserving deduplication without deleting visit history. The existing
limit of 128 visited states per block and its silent truncation behavior remain
unchanged.
