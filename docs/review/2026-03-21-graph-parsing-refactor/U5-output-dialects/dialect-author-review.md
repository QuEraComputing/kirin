# U5: Output & Dialects (kirin-prettyless + kirin-function) -- Dialect Author Review

## Workflow Trace

**Goal A**: Add graph printing to a custom dialect operation.

1. With `#[derive(PrettyPrint)]`, graph fields auto-render via `Document::print_digraph()` or `Document::print_ungraph()`. No manual work needed for standard `digraph ^name(...) { ... }` syntax.
2. For projected format, the derive generates calls to `print_ports_only()`, `print_captures_only()`, `print_digraph_body_only()` etc., matching the `{field:projection}` syntax from parsing.
3. The projection printer methods are symmetric with their parser counterparts, ensuring roundtrip fidelity.

**Goal B**: Understand kirin-function as a reference dialect implementation.

1. `FunctionBody`, `Lambda`, `Call`, `Return`, `Bind` -- each is a single struct with derives.
2. `#[chumsky(format = "...")]` on each struct defines the complete surface syntax.
3. `Lexical` and `Lifted` enums compose the operations with `#[wraps]` -- no boilerplate.
4. Interpreter impls are hand-written in `interpret_impl.rs` (not derived), showing the manual path.

## Findings

### [P2] [likely] Finding -- `print_digraph` / `print_ungraph` share header and body patterns

`ir_render.rs` contains `print_digraph` (~35 lines) and `print_digraph_body_only` (~20 lines) with near-identical body rendering logic. Similarly for ungraph. The full `print_ungraph` method interleaves edge/node printing logic that is duplicated in `print_ungraph_body_only`. Extracting the body rendering into a shared helper would reduce the ~60 lines of duplicated ungraph interleaving logic.

**File**: `crates/kirin-prettyless/src/document/ir_render.rs:192-303, 469-543`

### [P3] [confirmed] Finding -- kirin-function `Bind` has no interpreter support

`Bind::interpret` returns `Err(Unsupported)`. This is documented by the error message, but a dialect author using `Lifted` as a reference would discover this only at runtime. A compile-time warning (e.g., via `#[deprecated]` on `Bind`) or a doc comment on the struct would be more discoverable.

**File**: `crates/kirin-function/src/interpret_impl.rs:118-135`

### [P1] [likely] Finding -- `PrettyPrint` trait requires `L: PrettyPrint` bound on every method

The `pretty_print` method signature requires `L: Dialect + PrettyPrint` and `L::Type: Display`. This means a dialect author implementing `PrettyPrint` manually must propagate these bounds even for operations that never print nested structures. This is correct for composability, but the bound cascade could confuse newcomers. The `PrettyPrintViaDisplay` marker trait helps simple cases, but the gap between "I just implement Display" and "I need the full PrettyPrint trait" could use better documentation.

**File**: `crates/kirin-prettyless/src/traits.rs:40-54`

## Domain Alignment

| Domain Concept | Printer Mapping | Fit |
|---|---|---|
| Function body (Region with blocks) | `print_region()` / `print_block()` | Natural -- standard MLIR-style block rendering |
| Lambda captures | Format string `captures({captures})` + derive | Natural -- captures are SSAValues printed as `%name` refs |
| Function signature `(T, ...) -> T` | `print_signature()` + `Signature<T>::Display` | Natural -- standard PL function type syntax |
| DiGraph body (nodes + yield) | `print_digraph()` with topological node iteration | Natural -- petgraph node_references gives topo order for DAGs |
| UnGraph body (interleaved edges/nodes) | `print_ungraph()` with BFS edge interleaving | Natural -- edges printed before their consuming nodes |
| Pipeline cross-stage rendering | `RenderDispatch` + `PipelineDocument` | Natural -- type-erased dispatch enables heterogeneous stage printing |

## Strengths

- kirin-function is an excellent reference dialect: 5 operations, two composition patterns (`Lexical` vs `Lifted`), covering Region, captures, signatures, and terminators. A dialect author can study this one crate to learn the full dialect author workflow.
- The `PrettyPrintViaDisplay` marker trait eliminates boilerplate for simple types -- a dialect author with a `Display` impl gets `PrettyPrint` for free.
- The projection printer methods (`print_ports_only`, `print_digraph_body_only`, etc.) are cleanly symmetric with the parser projections, making roundtrip correctness structurally obvious.
- The `RenderBuilder` API (`node.render(&stage).config(...).globals(...).into_string()`) is ergonomic and discoverable.
