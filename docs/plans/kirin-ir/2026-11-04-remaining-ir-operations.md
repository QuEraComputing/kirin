# Remaining IR operations in the `kirin-ir` `Rewriter`

Branch: `alex/remaining-ir-operations`, off `rust`.

Adds the `Rewriter` operations needed for code motion and inlining: moving
statements and blocks, splicing whole bodies, copying bodies, and (as an
attempt) inlining a call.

## Background

### What exists

| Body | Operations |
|---|---|
| block body | `erase_statement`, `insert_before` / `insert_after` (new statements only), `replace_statement`, `insert_terminator` |
| CFG | `append_block`, `insert_block_before` / `_after` (new empty blocks only), `erase_block`, `split_block`, `merge_blocks`, `retarget_successor`, `insert_block_arg`, `erase_block_arg` |
| values | `replace_operand`, `replace_all_uses`, `replace_results`, `new_result` |
| `DiGraph` | `insert_node`, `replace_yield` (removal through `erase_statement`) |
| `UnGraph` | `insert_ungraph_node`, `insert_ungraph_edge` (removal through `erase_statement`) |

Nothing moves an existing statement or block, and nothing copies IR. The one
operation that moves IR is `merge_blocks`, which appends one block's
statements to the end of another block of the same CFG.

### Cost model

Each statement stores its parent block (`StatementParent::Block`), and each
block stores its parent CFG (`BlockParent::CFG`). Values store their defining
statement or block, and uses store the using statement. So:

- moving a **statement** costs O(1): relink it, change its parent;
- moving a **block** costs O(1), and its statements don't change;
- moving the **statements of a block** into another block costs O(number
  moved), one parent write each (this is why `split_block` costs O(statements
  after the split point));
- moving a **whole CFG's blocks** costs O(number of blocks).

Ids never change when IR moves, so uses stay valid for free. A copy creates new
ids and must map every operand, which costs O(size of what is copied).

### Comparison with MLIR

| MLIR (`RewriterBase`, `Region`, `Operation`) | Kirin today | This plan |
|---|---|---|
| `moveOpBefore` / `moveOpAfter` | none | step 2 |
| `Block::moveBefore` | none | not in this plan (step 3, dropped) |
| `inlineBlockBefore(source, dest, before, args)` | `merge_blocks` (end of block, same CFG only) | step 4 |
| `inlineRegionBefore(region, parent, before)` | none | step 5 |
| `Operation::clone`, `Region::cloneInto` with `IRMapping` | none | step 6 |
| `inlineCall` + `DialectInlinerInterface` | none | step 7 (attempt) |
| `replaceUsesWithIf`, `replaceAllUsesExcept` | `replace_all_uses` only | step 1 |
| `Operation::isBeforeInBlock` | none | not in this plan (dominance comes later) |
| `Value::setType` | none | not in this plan |
| `replaceOpWithNewOp` | `replace_statement` (same result count) | not needed |
| `eraseOpResults` | `remove_result` deferred | not in this plan |

## Steps (pause for review after each)

Each step adds the operation to `Rewriter`, its rejections as `RewriteError`
variants where needed, its `MutationEvent`, tests in `kirin-ir`, and its row in
the operation table of `docs/design/rewrite-engine.md`.

### 1. Replace only some uses

```rust
rewriter.replace_uses_if(old, new, |stage: &StageInfo<L>, site: Use| ...)?;
rewriter.replace_all_uses_except(old, new, stmt)?;
```

MLIR's `replaceUsesWithIf` / `replaceAllUsesExcept`. Like `replace_all_uses`,
but only the use sites the predicate accepts, or all but those of `stmt`. The
predicate gets the stage, so it can look up the using statement's parent or
neighbors; it runs on every use before anything is changed. CSE
and code motion need this when only some uses may change, e.g. only the uses
after a point. Same events as `replace_all_uses`, counting only the changed
slots.

Cost: O(uses of `old`).

### 2. Move a statement

```rust
rewriter.move_before(stmt, anchor)?;
rewriter.move_after(stmt, anchor)?;
```

Same anchor rules as `insert_before` / `insert_after`: `move_before` a
terminator appends to the end of that block's statements; nothing can go after
a terminator. The anchor may be in any block, of any CFG or statement-owned
body.

Example, hoisting out of an `scf.if` arm:

```
^entry:                          ^entry:
  %r = if %c then {                %t = neg %x     // moved
    %t = neg %x          ==>       %r = if %c then {
    yield %t                         yield %t
  } ...                            } ...
```

Rejects:
- `stmt` or `anchor` unknown;
- `stmt` is a terminator (terminators are not in the statement chain);
- `stmt` or `anchor` is a graph member;
- the anchor is inside `stmt`'s own nested bodies (moving an `scf.if` into its
  own arm would make it contain itself).

Not checked, like `insert_before`: that every use of `stmt`'s results still
comes after it. That is the caller's job.

Records a new `MutationEvent::MovedStatement { stmt }`. `ErasedStatement` +
`InsertedStatement` would wrongly say the statement was tombstoned.

**Walk check.** Moving the visited statement breaks pre-order, post-order and
BFS, which find the next statement through the visited one. Today the walk
only notices the `merge_blocks` case (the visited statement's old block was
erased). This step adds a check for moves, read from the `Rewriter`'s event
log, so nothing new is stored in the IR:

- Every operation that moves IR records an event naming what moved:
  `MovedStatement { stmt }` here, and the events of steps 3–5.
- `WalkCursor` gets a variant of `next` that takes the `Rewriter` instead of
  the stage. At each visit the cursor remembers the visited statement, its
  block, and the length of the event log. On its next step it scans only the
  events recorded since, and reports `WalkError::VisitedMoved` if one moved the
  visited statement or its block. When nothing was recorded, that is one
  length comparison.
- `walk_mut` uses the new variant. The read-only `walk` and `schedule` keep
  `next(stage)`, since nothing moves during them.
- The reverse BFS orders allow moves (their order is built at the start), so
  they skip the check.
- `split_block` records no move event, so the split the contract allows still
  passes. Moving an ancestor of the visited statement is not checked.

Cost: O(1), plus O(nesting depth) for the cycle check.

### 3. Move a block within its CFG (dropped)

Not added. A CFG's block order has one semantic effect, choosing the entry
block (the first in the list); otherwise it only changes printing and
iteration order. A new entry block can already be made with
`insert_block_before` plus a branch, and steps 5 and 7 place blocks while
splicing, so nothing needs a separate move. Add it when block layout starts
to matter (e.g. a backend or an analysis seeded in reverse postorder).

### 4. Splice a block's statements into another block

```rust
rewriter.splice_block_before(source, anchor, &arg_replacements)?;
rewriter.splice_block_after(source, anchor, &arg_replacements)?;
```

MLIR's `inlineBlockBefore`. Moves `source`'s statements (not its terminator)
next to `anchor`, and redirects uses of `source`'s arguments to
`arg_replacements`. The anchor rules are those of `move_before` /
`move_after`. `splice_block_after` is for step 7, where the callee's entry is
spliced after the call, the last statement of an unterminated block. `source` is left empty, keeping its arguments and
terminator, and is not erased: the caller decides what to do with it.
Generalizes `merge_blocks`: any position, in any block of any CFG or
statement-owned body. Records a new `SplicedBlock { source, dest }`, which the
walk check reads as "every statement of `source` moved". Afterwards a spliced
statement's parent is `dest`, so the check also takes the visited statement's
parent from before the visit, and compares it with `source`.

Rejects an anchor in `source` or nested in one of its statements
(`SplicedIntoItself`).

Example, folding `scf.if` with a known condition: splice the `then` arm in
front of the `if`, replace the `if`'s results with the arm's `yield` values,
erase the `if`.

Cost: O(statements moved).

### 5. Splice a whole CFG into another

```rust
rewriter.splice_cfg_after(source, anchor_block)?;
```

MLIR's `inlineRegionBefore`. Moves every block of `source` into `anchor_block`'s
CFG, right after `anchor_block`. Branches between the moved blocks stay valid,
since they all move together. `source` is left empty and not erased; its
owner (e.g. the callee function) still names it, and the caller decides
whether to erase the owner. Records `ChangedBlockList` for both CFGs and a new
`SplicedCfg { source, dest }`, which the walk check reads as "every block of
`source` moved".

Cost: O(blocks moved). This is the move-instead-of-copy path for inlining a
callee's last use.

### 6. Copy a statement or a CFG

```rust
let mut map = IrMap::default();           // MLIR's IRMapping
let copy = rewriter.clone_before(stmt, anchor, &mut map)?;
let blocks = rewriter.clone_cfg_after(source, anchor_block, &mut map)?;
```

A deep copy with fresh ids: statements, results, blocks, block arguments and
nested bodies, including `DiGraph` and `UnGraph` bodies. A copied graph gets
new ports, copied members and yields, and its topology is rebuilt from the
copied operands with the helpers `insert_node` / `insert_ungraph_node`
already use. Every operand and branch target inside the copy is mapped
through `map`. A value from outside the copied IR keeps pointing at the
original. The caller can pre-seed `map`, e.g. to map the callee's entry
arguments to the call's operands. `map` afterwards holds old → new for every
value and block copied, so the caller can find the copied `ret`s.

Cost: O(size of what is copied).

### 7. Inline a call (attempt)

An inliner in `kirin-function`, which knows its own call and `ret` ops, built
from steps 4–6 and the existing `kirin-ir` operations. It takes a closure that
builds the branch op, since `kirin-function` doesn't know which branch op a
language uses. It inlines `CallSpecialized` calls within the same stage; the
other call kinds first need resolving to a specialization. For a call `c` in block `B`:

1. `split_block` after `c`. The continuation block `C` gets the rest of `B`.
2. Copy the callee's CFG between `B` and `C`, mapping its entry arguments to
   `c`'s operands (step 6). For the callee's last use, splice instead (step 5).
3. Splice the copied entry block into `B` (step 4), or branch to it.
4. Replace each copied `ret v` with a branch to `C` passing `v`.
5. Give `C` one argument per result of `c`, redirect `c`'s results to them, and
   erase `c`.

Cost per call: O(statements after `c` in `B`) for the split, plus O(callee
size) for the copy. Inlining a block's calls from last to first makes the
splits cost O(block size) in total, since each statement moves at most once.

### 8. Docs

`Rewriter` module docs, the design doc's operation table, and the
`MutationEvent` count in Gap 3. The `kirin_ir::walk` contract gets a line on
the new moves.

## Decisions

1. Moves record new events naming what moved: `MovedStatement`,
   `SplicedBlock`, `SplicedCfg`.
2. The walk checks for a moved visited statement or block by reading the
   `Rewriter`'s event log (step 2). Ancestors are not checked.
3. Moving a block within its CFG is not added (step 3).
4. A block's statements can be spliced into any block.
5. Splicing leaves the source block or CFG empty and does not erase it.
6. Copying includes graph bodies.
7. Inlining lives in `kirin-function`, with a closure building the branch.
8. Only `CallSpecialized` within the same stage is inlined.
9. Of the extra MLIR operations, only `replace_uses_if` /
   `replace_all_uses_except` (step 1). Their predicate gets the stage.
