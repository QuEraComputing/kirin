# U1: Core IR (kirin-ir) -- Code Quality Review

## Clippy / Lint Findings

### [P2] [confirmed] #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)] at builder/digraph.rs:84, ungraph.rs:77, block.rs:93, region.rs:31
Root cause: All four builders use a `new(self) -> Id` pattern where `new` consumes the builder and returns a node ID, not `Self`. Clippy fires because `new` conventionally returns `Self`. Removable: yes. Fix: Rename `new()` to `build()` or `finish()` across all four builders. This eliminates all four suppressions with zero semantic change. Estimated effort: low (rename + update call sites).

### [P3] [confirmed] #[allow(clippy::unit_cmp)] at signature/semantics.rs:61, :97
Root cause: `Signature<T, C = ()>` -- when `C = ()`, `call.constraints() == cand.constraints()` compares two `&()` references, triggering `unit_cmp`. Removable: yes. Fix: Guard the comparison with `if std::mem::size_of::<C>() > 0` or specialize the `C = ()` case. Alternatively, use `matches!()` or skip the check when `C: Default`. Low priority since it only fires on the default constraint type.

## Duplication Findings

### [P1] [confirmed] DiGraphBuilder vs UnGraphBuilder -- builder/digraph.rs:1-230 vs builder/ungraph.rs:1-338
Lines duplicated: ~100 (port allocation, name-to-index map construction, replacement resolution, and replacement application are structurally identical). The port+capture allocation loop (lines digraph:89-113, ungraph:82-105), name-to-index map (digraph:115-132, ungraph:107-124), and SSA replacement scan+apply (digraph:134-183, ungraph:126-181) are near-copies differing only in `PortParent::DiGraph` vs `PortParent::UnGraph` and the label string. Suggested abstraction: Extract a `GraphBuilderCommon` helper struct or free functions `alloc_ports()`, `build_name_maps()`, `resolve_and_replace()` parameterized on graph kind. Lines saved: ~80-100.

### [P2] [confirmed] DiGraphInfo vs UnGraphInfo -- node/digraph.rs vs node/ungraph.rs
Lines duplicated: ~40 (identical accessor methods: `id()`, `parent()`, `name()`, `ports()`, `edge_count()`, `edge_ports()`, `capture_ports()`, `GetInfo` impl). Suggested abstraction: A `GraphInfo<Dir>` generic or a shared `GraphHeader` struct containing common fields. Lines saved: ~35.

## Rust Best Practices

### [P2] [likely] Missing #[must_use] on all builder types
`DiGraphBuilder`, `UnGraphBuilder`, `BlockBuilder`, `RegionBuilder`, `PlaceholderBuilder` -- all are consumed-builder patterns where silently discarding the builder loses work. Adding `#[must_use = "builder does nothing unless .new() is called"]` prevents silent drops.

### [P3] [uncertain] `language.rs` TODO comment (line 1)
`// TODO: use Cow<'a, str>` -- stale TODO that may not be actionable. Consider removing or filing a tracking issue.

## Strengths

- Error types (`PipelineError`, `StagedFunctionError`, `SpecializeError`) are well-structured with `Display` and `Error` impls.
- Builder API is ergonomic with method chaining.
- `debug_assert!` guards on `port_name`/`capture_name` ordering are good defensive practice.
- `identifier!` macro reduces boilerplate for ID newtypes consistently.
