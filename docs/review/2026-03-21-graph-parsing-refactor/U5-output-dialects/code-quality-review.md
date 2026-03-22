# U5: Output & Dialects (kirin-prettyless + kirin-function) -- Code Quality Review

## Clippy / Lint Findings

No `#[allow]` or `#[expect]` annotations found in either crate. Clean from a lint-suppression perspective.

## Duplication Findings

### [P1] [confirmed] print_ungraph() vs print_ungraph_body_only() -- ir_render.rs:233-302 vs :494-543
Lines duplicated: ~40. The body-rendering logic (edge-result-to-stmt map, printed-edges tracking, interleaved edge/node printing) is nearly identical in both methods. The only difference is that `print_ungraph()` adds the header and braces. Suggested abstraction: Extract `render_ungraph_body_inner()` and call it from both methods. Lines saved: ~35.

### [P1] [confirmed] print_digraph() vs print_digraph_body_only() -- ir_render.rs:192-230 vs :469-492
Lines duplicated: ~20. Same pattern: the body-rendering logic (node iteration + yield) is duplicated. Suggested abstraction: Extract `render_digraph_body_inner()`. Lines saved: ~15.

### [P1] [confirmed] FunctionBody vs Lambda -- interpret_impl.rs:9-78
Lines duplicated: ~60. `SSACFGRegion` and `Interpretable` impls for `FunctionBody` and `Lambda` are structurally identical (both access `self.body.blocks(stage).next()`). Suggested abstraction: A generic helper function `interpret_region_body(body: &Region, interp) -> Result<Continuation, Error>` or a blanket impl over a `HasRegionBody` trait. Lines saved: ~40.

### [P2] [likely] print_block() body logic vs print_block_body_only() -- ir_render.rs:92-139 vs :556-571
Lines duplicated: ~15. The statement+terminator iteration pattern is repeated. Suggested abstraction: Extract `render_block_body_inner()`. Lines saved: ~12.

## Rust Best Practices

### [P2] [likely] ir_render.rs at 604 lines -- decomposition opportunity
This file contains rendering logic for SSA refs, signatures, statements, blocks, regions, digraphs, ungraphs, staged functions, specialized functions, and projection helpers. Suggested split: `ir_render_graph.rs` (digraph/ungraph rendering), `ir_render_function.rs` (staged/specialized function rendering), keeping core block/statement/region rendering in `ir_render.rs`.

### [P2] [likely] Missing #[must_use] on RenderBuilder
`RenderBuilder` is a builder pattern that is useless if dropped without calling `into_string()`, `print()`, or `bat()`. Adding `#[must_use = "call .into_string(), .print(), or .bat() to produce output"]` prevents silent drops.

### [P3] [uncertain] Call::interpret at 70 lines (interpret_impl.rs:137-248)
The `Call::interpret` method chains 6 fallible lookups with error construction at each step. While correct, the method is long. Consider extracting the lookup chain into a helper `resolve_callee(interp, target, stage_id) -> Result<SpecializedFunction, Error>` to reduce the method to ~20 lines.

## Strengths

- `PrettyPrintViaDisplay` marker trait is an elegant way to provide blanket `PrettyPrint` for `Display` types without manual impls.
- `RenderBuilder` provides a clean fluent API for configuring rendering output.
- Projection helpers (`print_ports_only`, `print_captures_only`, `print_yields_only`, `print_*_body_only`) enable format-string-driven rendering from the derive macro.
- Error handling in `Call::interpret` is thorough, mapping each resolution failure to a specific `StageResolutionError` variant with context.
