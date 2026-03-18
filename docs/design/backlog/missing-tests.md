# Missing Test Cases — Graph Body Parser/Printer

Identified during review of the digraph/ungraph parser implementation (2026-03-17).

## Ungraph body roundtrip

No test parses a full `ungraph ^ug0(...) { edge ...; node ...; }` body through the emit → print → reparse path. The current ungraph roundtrip tests only cover individual statement-level parsing (`wire`, `node_a`).

**What to test**: parse an ungraph body containing edge statements and node statements, print it, reparse, compare.

**Files**: `tests/roundtrip/ungraph.rs`

## Nested compound node roundtrip

No test for a compound node containing an inner ungraph body:

```
%out = compound(%e0) {
  ungraph ^inner(%ip0: T) {
    edge %w = wire;
    node_a(%ip0, %w);
  }
}
```

**What to test**: parse → emit → print → reparse for nested graph bodies.

**Files**: `tests/roundtrip/ungraph.rs` or `tests/roundtrip/digraph.rs`

## Pipeline-level graph roundtrip

No test for graph bodies as function bodies in the full pipeline parse:

```
stage @A fn @f(f64) -> f64;
specialize @A fn @f(f64) -> f64 {
  digraph ^dg0(%p0: f64) { ... yield %r; }
}
```

**What to test**: `roundtrip::assert_pipeline_roundtrip` with a language that has a graph-body function variant.

**Files**: `tests/roundtrip/digraph.rs`

## Digraph cycle roundtrip

The forward-reference test uses a DAG (definitions just in reversed order). No test with an actual cycle:

```
digraph ^dg0(%input: Signal) {
  %a = delay %b;
  %b = gain %a;
  yield %a;
}
```

**What to test**: parse → emit → print → reparse for a cyclic digraph.

**Files**: `tests/roundtrip/digraph.rs`

## Multiple captures

Existing tests use 0 or 1 capture. No test with 2+ captures to verify the capture clause parser and builder handle multiple entries.

**Files**: `tests/roundtrip/digraph.rs`, `tests/roundtrip/ungraph.rs`

## Empty graph (no ports)

No test for `digraph ^dg0() { ... }` or `ungraph ^ug0() { ... }` — graphs with zero ports.

**Files**: `tests/roundtrip/digraph.rs`, `tests/roundtrip/ungraph.rs`
