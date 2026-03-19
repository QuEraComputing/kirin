# Compiler Engineer — Cross-Cutting Review: kirin-ir

## Build Graph

**Dependencies:** `bon`, `indexmap`, `petgraph`, `rustc-hash`, `smallvec`, plus optional `kirin-derive-ir` and `serde`.

The dependency set is lean for a core IR crate. Key observations:

- **`bon` is a heavy proc-macro dependency** used only for builder patterns on `Pipeline` methods (`add_stage`, `function`, `staged_function`, `define_function`). `bon` pulls in `darling 0.20` (via its own proc-macro crate), which is a *different* version from the workspace's `darling 0.23`. This means the workspace compiles two darling versions. The `bon` builders are convenient but could be replaced with hand-written builders on these four methods to eliminate the extra darling version and reduce proc-macro compilation time. Since `kirin-ir` is the root of the entire dependency graph, any compilation cost here cascades to every downstream crate.

- **Feature gating is well-structured.** The `derive` feature gates `kirin-derive-ir`, and `serde` is optional. The `derive` feature defaults to on, which is reasonable for end users but means CI always compiles the derive proc-macro even when testing non-derive functionality.

- **Re-export surface is large but necessary.** `lib.rs` re-exports ~50 symbols from internal modules. This is appropriate for a core IR crate where users need direct access to node types. The `prelude` module keeps the common subset to 15 symbols.

- **No circular dependency risk.** `kirin-ir` sits at the bottom of the crate graph with only `kirin-derive-ir` as an optional dependency flowing upward.

## Scalability

- **Tuple-based dialect dispatch (`StageDispatch`):** The `type Languages = (L1, (L2, (L3, ())))` pattern is a compile-time recursive type-level list. With N dialects in a stage enum, this creates N nested tuple types and N `StageDispatch` impl instantiations. At 10 dialects this is fine; at 50 it becomes noticeable in compile times due to trait solver recursion depth. The runtime cost is O(N) linear scan per dispatch (try dialect 1, then 2, etc.), which is acceptable since N is small in practice. However, there is no short-circuit optimization for the common case where the stage is a single-dialect `StageInfo<L>`.

- **`Dialect` trait has 19 supertraits** (14 field iterators + 5 properties + `Clone` + `PartialEq` + `Debug`). Each `#[derive(Dialect)]` generates 21 trait impls. For an enum with V variants, each field-iterator impl generates a match with V arms. With 50 dialects, each with an average of 10 variants, that is 50 * 21 * 10 = 10,500 match arms in generated code. This is manageable but the per-dialect fixed cost of 21 impls is high.

- **Arena-based storage scales well.** `StageInfo` uses `Arena<K, V>` for all node types, giving O(1) lookups by ID. The `InternTable` for symbol resolution is also O(1). No scalability concerns with the data structures themselves.

- **`Pipeline` function lookup is O(1)** via `FxHashMap<GlobalSymbol, Function>`. Good.

## Error Quality

- **`AsBuildStage` diagnostic is excellent.** The `#[diagnostic::on_unimplemented]` attribute on `AsBuildStage` provides a clear message when users accidentally pass `&mut StageInfo` instead of `&mut BuilderStageInfo`: "use `stage.with_builder(|b| { ... })` to get a `&mut BuilderStageInfo` for construction". This is a model for other traits in the crate.

- **`StageDispatchMiss` and `StageDispatchRequiredError` provide structured errors** for stage resolution failures, with both `MissingStage` and `MissingDialect` variants. Good.

- **`PipelineError` and `PipelineStagedError` give actionable messages** (e.g., `DuplicateFunctionName`, `UnknownFunction`).

- **`FinalizeError` distinguishes three failure modes** (`UnresolvedSSA`, `TestSSA`, `MissingType`), each identifying the specific SSA value. Good for debugging.

- **`StageMeta::from_stage_name` returns `Result<Self, String>`** -- the `String` error type is weak. A structured error type with the attempted name and `declared_stage_names()` for suggestions would be better, though the downstream `FunctionParseError` in `kirin-chumsky` does provide typo suggestions via `strsim`.

- **Missing: `#[diagnostic::on_unimplemented]` on other key traits.** `HasStageInfo<L>`, `Dialect`, `StageMeta` would all benefit from custom diagnostics when users hit trait bound failures. Currently, a missing `HasStageInfo<LangA>` impl on a stage enum produces a generic "trait not satisfied" error.

## Compilation Time

- **19 HRTB supertraits on `Dialect`** (`for<'a> HasArguments<'a>`, etc.) are a significant trait-solver cost. Each time the compiler checks `L: Dialect`, it must verify all 19 supertraits. For generic code with multiple `L: Dialect` bounds, this multiplies.

- **`bon` proc-macro overhead.** The `#[bon::bon]` attribute on `Pipeline` impl blocks invokes a proc-macro for 4 methods. This adds compile time on the critical path. The generated builder types also participate in type inference, adding solver work.

- **Generics on `Pipeline<S>`** are minimal (one type parameter). The `staged_function` and `define_function` methods add `L: Dialect` and `S: HasStageInfo<L>` which is reasonable.

- **`StageDispatch` trait has 6 generic parameters** (`S, L, Tail, A, R, E`). The tuple impls are deeply nested. With N dialects, the compiler resolves N levels of `(L, Tail)` recursion. This is compile-time linear but contributes to slow builds in deeply generic interpreter code.

## Summary

- **P2** [confirmed] `bon` dependency brings a second `darling` version into the workspace, adding unnecessary proc-macro compile time to the critical-path crate — `crates/kirin-ir/Cargo.toml:7`
- **P2** [likely] Missing `#[diagnostic::on_unimplemented]` on `HasStageInfo`, `Dialect`, and `StageMeta` would significantly improve user-facing error messages — `crates/kirin-ir/src/stage/meta.rs:28`, `crates/kirin-ir/src/language.rs:103`
- **P3** [confirmed] `StageMeta::from_stage_name` returns `String` error -- a structured error type would enable better downstream diagnostics — `crates/kirin-ir/src/stage/meta.rs:84`
- **P3** [likely] 19 HRTB supertraits on `Dialect` contribute to trait-solver pressure; most dialects use only 2-3 field categories, but all 19 must be checked — `crates/kirin-ir/src/language.rs:103-128`
- **P3** [informational] Tuple-based `StageDispatch` is O(N) per dispatch; acceptable for small N but does not scale to large dialect counts — `crates/kirin-ir/src/stage/dispatch.rs:26-39`
