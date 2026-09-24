# `Owner` conflates stored state and executable work

The forward analysis uses `Owner<K>` for two roles: identifying stored analysis
state and identifying a computation that can be rerun.

| Variant | Identifies stored state | Can be scheduled for execution |
|---|---|---|
| `Function(context)` | Yes: function entry/return record | No |
| `Block { function, block }` | Yes: block entry/output facts | Yes |
| `Graph { function, graph }` | Yes: graph entry/output facts | Yes |

The roles overlap for blocks and graphs, but diverge for functions. The shared
driver nevertheless uses the same key type for its summary map and
`WorkItem::Analyze(key)`. Consequently, the types permit scheduling a function
record, and the forward adapter must reject it at runtime.

Renaming the existing enum to `WorkItem` would preserve the problem: its
`Function` variant still would not describe executable work.

The distinction to preserve is:

- **State identity:** which stored information changed?
- **Work identity:** which computation must run again?
- **Work item:** a queued request to execute that computation.

For example, a change to a callee's return information schedules the caller's
block. The dependency source and the scheduled computation need not have the
same identity type.

The design should separate executable work identities from storage identities,
so only blocks and graphs can enter the forward worklist. Dependency tracking
can then connect changes in stored information to runnable work. This is a type
boundary problem; a naming change alone cannot resolve it.

Sources: [`Owner`](../../crates/kirin-interpreter/src/engines/sparse_forward/interp.rs),
[`WorkItem`](../../crates/kirin-interpreter/src/fixpoint/traits.rs), and the
[fixpoint driver](../../crates/kirin-interpreter/src/fixpoint/solver.rs).
