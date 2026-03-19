# PL Theorist — Formalism Review: kirin-prettyless

## Abstraction Composability

### PrettyPrint trait: self-recursive with dialect parameter

`PrettyPrint` (`traits.rs:39-80`) is a trait with three methods:

1. `pretty_print` — default that delegates to `namespaced_pretty_print` with empty namespace.
2. `namespaced_pretty_print` — required method, takes a namespace filter.
3. `pretty_print_name` / `pretty_print_type` — projections for name-only or type-only rendering.

The trait is parameterized by `L: Dialect + PrettyPrint` in each method's generic context. This means the dialect type `L` must itself implement `PrettyPrint` to enable recursive rendering of nested structures (blocks, regions). The `L::Type: Display` bound enables type annotation printing.

The self-referential structure `L: Dialect + PrettyPrint` is solved by the same coinductive resolution that the interpreter uses for `L: Interpretable`. Each dialect's `PrettyPrint` impl can call `doc.render_block(...)` which internally uses `L::pretty_print`, creating the recursion.

### Document builder pattern

`Document<'s, L>` (`document/mod.rs`, `document/builder.rs`) wraps a `prettyless::Arena` allocator and provides domain-specific methods for rendering IR nodes (blocks, regions, SSA values, etc.). The `'s` lifetime is tied to the `StageInfo<L>` reference, ensuring the document cannot outlive the IR data.

The `Document` type acts as an **algebraic document builder** in the tradition of Wadler-Lindig pretty printers (Wadler, "A Prettier Printer", 2003; Lindig, "Strictly Pretty", 2000). The `prettyless` library provides the underlying `DocBuilder` algebra (text, line break, group, nest), and `Document` extends it with IR-specific combinators.

### RenderBuilder: staged configuration

`RenderBuilder<'n, 's, N, L>` (`traits.rs:105-164`) provides a builder API for rendering:

```
node.render(stage).config(c).globals(g).to_string()
```

This is the standard **staged builder** pattern. The four lifetime/type parameters (`'n` for node, `'s` for stage, `N` for node type, `L` for dialect) ensure type safety while allowing flexible composition. The builder collects configuration, then produces output in a single call.

### PrettyPrintExt: blanket convenience

`PrettyPrintExt<L>` (`traits.rs:186-217`) provides `render` and `sprint` convenience methods via a blanket impl over `T: PrettyPrint`. This follows the **extension trait** pattern where the base trait (`PrettyPrint`) defines the core operation and the extension trait adds convenience methods that require additional context (`StageInfo<L>`).

The blanket impl is unconditional — any `PrettyPrint` type gets `PrettyPrintExt` for free. This is a clean separation: `PrettyPrint` defines *how* to render (format-dependent), while `PrettyPrintExt` defines *where* to render (context-dependent).

### RenderDispatch: type-erased pipeline rendering

`RenderDispatch` (`pipeline.rs:38-52`) provides type-erased rendering for heterogeneous pipeline stages:

```rust
trait RenderDispatch {
    fn render_staged_function(&self, sf, config, global_symbols) -> Result<Option<String>>;
}
```

The blanket impl for `StageInfo<L>` (`pipeline.rs:56-77`) handles the common case. User stage enums delegate via match arms. This is the **visitor pattern** with type erasure: the inner dialect type is hidden behind `RenderDispatch`, allowing `PipelineDocument` to iterate over stages without knowing their concrete types.

The `Option<String>` return type handles the case where a staged function doesn't exist in a particular stage. This is a clean encoding of partial functions.

### Pipeline printing hierarchy

Three levels of printing granularity:

1. `PrettyPrintExt` — single node (statement, block, region)
2. `PrintExt` on `Function` — one function across all stages
3. `PipelinePrintExt` on `Pipeline<S>` — all functions in the pipeline

Each level composes: `PipelinePrintExt` uses `PipelineDocument::render_function` which uses `RenderDispatch::render_staged_function` which uses `Document::render`. The composition is strictly top-down with no circular dependencies.

## Literature Alignment

### Wadler-Lindig pretty printing

The crate uses `prettyless`, which implements a variant of Wadler's "A Prettier Printer" algorithm. The `Arena` allocator and `DocBuilder` type correspond to the standard document algebra:

- `text(s)` — literal text
- `line()` / `softline()` — line breaks (mandatory or optional)
- `group(d)` — try to flatten `d` onto one line
- `nest(i, d)` — indent `d` by `i` spaces

This is the standard formalism for pretty printing in functional programming, used by GHC, OCaml, and Rust's own `fmt` infrastructure.

### MLIR printing conventions

The pretty printer follows MLIR's textual format conventions:

- SSA values: `%N` (numeric) or `%name` (symbolic)
- Block labels: `^N` or `^name`
- Type annotations: `: type` suffix
- Regions: `{ ... }` with block headers
- Namespaced operations: `dialect.operation`

The `namespaced_pretty_print` method with a `&[&str]` namespace parameter matches MLIR's convention where operations are printed with their dialect namespace prefix.

### The roundtrip property

The documented target invariant is `parse(sprint(ir)) == ir`. This is the **retraction** property from category theory: `parse . print` is the identity on the IR domain. The `sprint` function is a section (right inverse) of `parse`. This property is maintained by using the same format string for both `#[derive(HasParser)]` and `#[derive(PrettyPrint)]`.

Note that the converse (`sprint(parse(text)) == text`) does not generally hold because the printer canonicalizes whitespace, formatting, and symbolic names. The system implements a *partial isomorphism* (prism in lens terminology) rather than a full isomorphism.

## Semantic Ambiguity

### `PrettyPrint::pretty_print_name` and `pretty_print_type` defaults

Both `pretty_print_name` and `pretty_print_type` default to `pretty_print` (`traits.rs:58-79`). This means that for types that don't override these methods, the "name view" and "type view" are identical to the full view. For types like `ResultValue` where name and type are distinct concepts, the defaults are incorrect and must be overridden. There is no compile-time check that types which appear in `{field:name}` or `{field:type}` format positions actually override these methods.

### `sprint` panics on render failure

`PrettyPrintExt::sprint` (`traits.rs:214-216`) calls `self.render(stage).to_string().expect("render failed")`. The `expect` will panic on render errors (e.g., missing IR nodes). The `render` builder API properly returns `Result`, but the shorthand `sprint` loses this error information. This is documented by convention but could surprise users who expect infallible printing.

### `RenderDispatch` returns `std::fmt::Error` not `RenderError`

`RenderDispatch::render_staged_function` returns `Result<Option<String>, std::fmt::Error>` (`pipeline.rs:47-51`), while the higher-level `PipelineDocument` uses `RenderError` (`pipeline.rs:100`). The error type mismatch between the trait and its consumer is bridged by a `From` conversion, but `std::fmt::Error` carries no diagnostic information — it is a unit error type. A staged function that fails to render produces an opaque error with no indication of what went wrong.

## Alternative Formalisms Considered

### 1. Pretty printer algebra: Wadler-Lindig vs. Hughes vs. Oppen

**Current**: Wadler-Lindig via `prettyless` (optimal line-breaking, arena-allocated documents).
**Alternative A**: Hughes-style (combinators with explicit `<>` composition, no global optimization).
**Alternative B**: Oppen's algorithm (streaming, linear-time, no document construction).

| Metric | Wadler-Lindig (current) | Hughes | Oppen |
|--------|------------------------|--------|-------|
| Output quality | Optimal | Heuristic | Good |
| Memory usage | O(document size) | O(document size) | O(1) streaming |
| Implementation complexity | Medium | Low | High |
| Composability | Good (algebraic) | Good (algebraic) | Poor (imperative) |
| Rust ecosystem support | `prettyless` | `pretty` crate | Custom needed |

Wadler-Lindig is the standard choice for compiler pretty printers where output quality matters and document size is bounded by program size.

### 2. Type erasure: RenderDispatch vs. dynamic dispatch vs. visitor

**Current**: `RenderDispatch` trait object with blanket impl and manual delegation.
**Alternative A**: `dyn RenderDispatch` trait objects stored in the pipeline.
**Alternative B**: Visitor pattern with accept/visit methods on stages.

| Metric | RenderDispatch (current) | dyn dispatch | Visitor |
|--------|------------------------|-------------|---------|
| Type safety | Full (blanket + match) | Partial (downcasting) | Full |
| Extensibility | Open (new stages) | Open | Open |
| Boilerplate | Medium (match arms) | Low | High (double dispatch) |
| Performance | Monomorphized | Virtual dispatch | Virtual dispatch |

The current approach is the right balance: blanket impls handle the common case, and manual match delegation handles the heterogeneous case. The `#[derive(RenderDispatch)]` macro further reduces boilerplate.

### 3. Namespace filtering: string array vs. type-level namespace

**Current**: `&[&str]` namespace array passed at runtime.
**Alternative A**: Type-level namespace via phantom types (e.g., `PrettyPrint<Namespace>`).
**Alternative B**: Newtype wrappers for namespaced types.

| Metric | String array (current) | Type-level | Newtype |
|--------|----------------------|-----------|---------|
| Type safety | None (runtime filtering) | Full | Full |
| Flexibility | High (dynamic composition) | Low (static) | Low |
| Performance | String comparison | Zero-cost | Zero-cost |
| Composability | Additive | Rigid | Per-namespace |

Runtime string filtering is the pragmatic choice for a framework where namespace composition is dynamic (determined by the pipeline configuration, not the type system).

## Summary

- [P2] [likely] `RenderDispatch` returns `std::fmt::Error` (no diagnostic info) while consumers use richer `RenderError` — `pipeline.rs:47-51`
- [P3] [confirmed] `pretty_print_name` / `pretty_print_type` defaults to full rendering; no compile-time enforcement of override for projected fields — `traits.rs:58-79`
- [P3] [confirmed] `sprint` panics on render failure instead of returning Result — `traits.rs:214-216`
- [P3] [informational] Three-level printing hierarchy (node, function, pipeline) composes cleanly with no circular dependencies — `traits.rs`, `pipeline.rs`
- [P3] [informational] Wadler-Lindig pretty printing via `prettyless` is the standard choice for compiler IR formatting — `document/builder.rs`
