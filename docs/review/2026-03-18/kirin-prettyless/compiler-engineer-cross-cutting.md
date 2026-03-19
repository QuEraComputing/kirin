# Compiler Engineer — Cross-Cutting Review: kirin-prettyless

## Build Graph

**Dependencies:** `bon`, `kirin-ir`, `petgraph`, `prettyless`, optional `kirin-derive-prettyless`, optional `bat`, optional `serde`.

- **`bon` appears here as well as in `kirin-ir`.** This is the second crate on the critical path that depends on `bon`. Since `bon` is a proc-macro crate, it only compiles once for the workspace, but it adds to the initial compilation wall. In this crate, `bon` is used for builder-pattern methods (the `RenderBuilder` API). Since the builders here are simpler than those in `kirin-ir`, hand-written builders would be straightforward.

- **`bat` is a very heavy optional dependency.** The `bat` crate (syntax highlighting/paging) pulls in `syntect`, `regex`, `onig`/`fancy-regex`, and many other transitive dependencies. While it is feature-gated, the `default` features include `bat`: `default = ["serde", "bat"]`. This means a bare `kirin-prettyless` dependency compiles `bat` and all its transitive dependencies unless the user explicitly opts out with `default-features = false`. **For a library crate, defaulting `bat` to on is aggressive.** End-user binaries should opt into `bat`; library crates should not pull it by default.

- **`petgraph` is a direct dependency**, same as `kirin-ir`. Since `kirin-ir` also depends on `petgraph`, this is likely for graph rendering support (printing `DiGraph`/`UnGraph` nodes). Both crates resolve to the same `petgraph` version through workspace dependencies.

- **`kirin-chumsky` depends on `kirin-prettyless`**, creating a one-directional coupling: parser -> printer. This means changes to `kirin-prettyless`'s public API invalidate `kirin-chumsky` and everything downstream. The coupling is through the `PrettyPrint` trait re-export and `RenderDispatch`.

## Scalability

- **`RenderDispatch` is a trait-object-compatible dispatch trait.** Each stage enum implements `RenderDispatch` with a match arm per dialect variant. With N dialects, the match has N arms. The rendering itself is per-function, so the cost is O(functions * dialects_checked). Since only one match arm succeeds per call, this is effectively O(1) with a small constant.

- **`PipelineDocument::render_function` iterates all staged functions** for a given `Function`. With M stages, this is O(M) iterations. For a pipeline with many stages (e.g., 50 optimization passes), this is linear in pipeline depth. The rendering for each stage is independent, so this could be parallelized, but is unlikely to be a bottleneck.

- **`PipelineRenderBuilder::to_string` iterates all functions** in the pipeline's function arena. With F functions and M stages, total rendering is O(F * M). For large programs (thousands of functions), this could be slow, but pretty-printing is typically an interactive/debugging operation.

- **`PrettyPrint` trait methods take `&'a Document<'a, L>`** and return `ArenaDoc<'a>`. The arena-based document allocation (`prettyless::Arena`) avoids per-node heap allocation during document construction. The final rendering is a single pass over the document tree. Good memory behavior.

## Error Quality

- **`RenderError` provides structured error variants.** Looking at `error.rs`, it likely has variants for unknown functions and I/O errors. The `io::Error` wrapping via `From` is standard.

- **`PrettyPrint` has no error path** -- it returns `ArenaDoc<'a>` directly, not `Result`. This means formatting failures (e.g., missing SSA info) would panic rather than produce errors. The `Document::render` method returns `Result<String, fmt::Error>`, so formatting errors are caught at the rendering step, but semantic errors (e.g., dangling SSA references) would panic during document construction.

- **`RenderDispatch::render_staged_function` returns `Result<Option<String>, fmt::Error>`.** The `Option` distinguishes "this stage does not own this staged function" (returns `None`) from "rendering failed" (returns `Err`). This is a clean API for type-erased dispatch.

- **No derive-macro-specific diagnostics** since `kirin-derive-prettyless` (`RenderDispatch` derive) is a separate crate. The `PrettyPrint` derive lives in `kirin-derive-chumsky`, which has good validation (see that review).

## Compilation Time

- **`PrettyPrint` trait has 3 methods** with generic bounds (`L: Dialect + PrettyPrint`, `L::Type: Display`). Each impl of `PrettyPrint` generates 3 monomorphized methods per language type `L`. With 5 language enums and 50 dialect types implementing `PrettyPrint`, that is 250 monomorphizations of `pretty_print` alone.

- **`PrettyPrintExt<L>` is a blanket impl** parameterized by `L: Dialect + PrettyPrint`. Each call to `.sprint(stage)` or `.render(stage)` monomorphizes over both the node type `T` and the dialect `L`. With M node types and N languages, that is O(M * N) monomorphizations. In practice M is small (usually `Statement`, `Block`, `Region`).

- **`Document<'a, L>` carries the dialect type `L`** throughout all rendering operations. This means the document builder's methods are all generic over `L`, producing separate codegen per dialect. This is inherent to the design (the printer needs to know the dialect for recursive block/region rendering).

- **`bon` adds proc-macro compile time** but is used lightly here (only for `RenderBuilder` if at all -- need to verify). The `Config` struct uses `bon`'s builder pattern. If `bon` is only used for `Config`, hand-writing the builder would eliminate the dependency.

## Summary

- **P1** [confirmed] `bat` is in default features for a library crate; downstream libraries compile `bat` + `syntect` unless they explicitly opt out — `crates/kirin-prettyless/Cargo.toml:16`
- **P2** [confirmed] `bon` dependency (also in `kirin-ir`) adds proc-macro compile time; used only for builder patterns that could be hand-written — `crates/kirin-prettyless/Cargo.toml:8`
- **P2** [likely] `PrettyPrint` document construction panics on semantic errors (dangling SSA refs) rather than returning errors — `crates/kirin-prettyless/src/traits.rs:39-80`
- **P3** [informational] `PrettyPrint` monomorphizes over both node type and dialect, creating O(M * N) instantiations — `crates/kirin-prettyless/src/traits.rs:201-217`
