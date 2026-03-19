# Compiler Engineer — Cross-Cutting Review: kirin-interpreter

## Build Graph

**Dependencies:** `kirin-ir` (with `derive` feature), optional `kirin-derive-interpreter`, `rustc-hash`, `smallvec`, `thiserror`.

- **`kirin-ir` is pulled with `features = ["derive"]`**, meaning `kirin-derive-ir` (a proc-macro crate) is always compiled for `kirin-interpreter`. This is correct since the interpreter crate needs `StageMeta` derive, but it means changes to `kirin-derive-ir` invalidate `kirin-interpreter` even if the derive output is unchanged.

- **Dev-dependencies are heavy.** The test suite pulls in `kirin-arith`, `kirin-cf`, `kirin-constant`, `kirin-function`, `kirin-interval`, `kirin-test-languages`, `kirin-test-utils`, and `insta`. This is 8 dev-dependencies, several of which have their own feature-gated sub-dependencies. This does not affect production builds but makes `cargo nextest run -p kirin-interpreter` slower to start. Not a real problem, just worth noting.

- **No dependency on `kirin-chumsky` or `kirin-prettyless`.** Good separation -- the interpreter crate is independent of parsing and printing.

- **`thiserror` is a reasonable choice** for `InterpreterError` and `StageResolutionError`. Since thiserror 2.x uses a proc-macro, it adds compile time, but the alternative (manual `Display`/`Error` impls) is worse for maintainability.

## Scalability

- **`DispatchCache` provides O(1) stage lookup.** Pre-computes entries per stage at construction time, indexed by stage ID. This is the right pattern for hot-path dispatch. The `by_stage: Vec<Option<Entry>>` wastes one `Option` per stage but that is negligible.

- **`dispatch_in_pipeline` uses `SupportsStageDispatch` which bottlenecks on the tuple-dispatch in `kirin-ir`.** The runtime cost is O(N) per call where N is the number of dialects in the stage enum. For the `StackInterpreter`, `DispatchCache` avoids repeated dispatch, but `AbstractInterpreter` also uses similar caching.

- **`Interpretable::interpret<L>` monomorphizes per language type `L`.** With 5 language enums each used with `StackInterpreter` and `AbstractInterpreter`, that is 10 monomorphizations of every `interpret` method in every dialect. For 50 dialects, that is 500 monomorphized functions just for `interpret`. The method bodies are typically small (match + delegate), so code size is manageable, but the monomorphization pressure on the linker is real.

- **`BlockEvaluator::eval_block<L>` also monomorphizes per `L`.** Same scaling concern. The `bind_block_args` default impl allocates a `Vec<SSAValue>` per call, which is an O(N) allocation where N is the block's argument count. For hot loops (abstract interpretation fixpoints), this could become a bottleneck. A `SmallVec` or pre-allocated scratch buffer would help.

- **`FrameStack` and `Frame` are allocated per call.** The stack interpreter pushes/pops frames for each function call. Frame allocation is heap-based. For deep call stacks (recursive functions), this is O(depth) allocations. Not a scalability issue for typical programs but worth noting for large-scale analysis.

## Error Quality

- **`InterpreterError` is well-structured with 9 variants.** Each variant carries enough context to diagnose the problem: `UnboundValue(SSAValue)`, `ArityMismatch { expected, got }`, `StageResolution { stage, kind }`, etc.

- **`StageResolutionError` has 8 variants** covering every resolution failure mode. The `MissingFunction { function }` and `UnknownTarget { name }` variants are particularly helpful since they identify the specific function that could not be resolved.

- **`MissingEntryError` has 3 variants** with clear names: `EntryBlock`, `BlockTerminator`, `FunctionEntry`. The convenience constructors (`missing_entry_block()`, etc.) are nice API design.

- **The `Custom(Box<dyn Error>)` variant allows user-defined errors** without cluttering the framework error type. Good extensibility pattern.

- **`StageAccess::active_stage_info` panics instead of returning `Result`.** The doc comment says "Panics if the active stage does not contain a `StageInfo<L>`". This is intentional (the caller is expected to know which stage they are in), but a mismatched `L` type parameter will produce a panic with the message "active stage does not contain StageInfo for this dialect" -- which does not name the dialect or stage. Including `std::any::type_name::<L>()` in the panic message would be trivially helpful.

- **`Interpretable` derive validation is good.** When a variant lacks `#[wraps]`, the error message names the offending variant(s) and suggests the fix: "Either implement `Interpretable` manually, or wrap each variant with `#[wraps]`."

## Compilation Time

- **`Interpreter<'ir>` is a blanket trait** auto-implemented for all `BlockEvaluator<'ir>`. This means the compiler must prove `T: BlockEvaluator<'ir>` to resolve `T: Interpreter<'ir>`, which chains through `T: ValueStore + StageAccess<'ir> + 'ir`. Three trait proofs per `I: Interpreter<'ir>` bound.

- **`Interpretable<'ir, I>` has `L` on the method**, not the trait. This is documented as an intentional E0275 workaround. The cost is that each call site `dialect.interpret::<L>(interp)` generates a monomorphized function. The alternative (trait-level `L`) was infeasible, so this is accepted.

- **Generated `Interpretable` impls add per-wrapper-type where clauses.** For an enum with 5 `#[wraps]` variants, the where clause has 5 `InnerType: Interpretable<'__ir, __InterpI>` predicates plus the method-level `__InterpL: Interpretable<'__ir, __InterpI> + '__ir`. This is 6 predicates per impl. With 50 dialects, the aggregate where clause complexity is manageable but contributes to incremental compilation invalidation (any change to `Interpretable` signature invalidates all impls).

- **`StageAccess` has 5 provided methods** with generic bounds, each producing a monomorphized function per call site. The `in_stage<L>()` and `with_stage<L>()` methods construct `Staged<'_, 'ir, Self, L>` which has 4 type parameters. The `Staged` type is used as a temporary builder, so it should not be stored in data structures, but the compiler still generates vtable-like metadata for trait method dispatch.

## Summary

- **P2** [likely] `bind_block_args` allocates `Vec<SSAValue>` per call; in hot fixpoint loops this is an avoidable allocation. Use `SmallVec` or scratch buffer — `crates/kirin-interpreter/src/block_eval.rs:44`
- **P2** [confirmed] `active_stage_info` panic message does not include the dialect type name, making it hard to diagnose stage/dialect mismatches — `crates/kirin-interpreter/src/stage_access.rs:37`
- **P3** [informational] `Interpretable::interpret<L>` monomorphizes per language type, creating O(dialects * languages) instantiations — `crates/kirin-interpreter/src/interpretable.rs:17`
- **P3** [informational] Dev-dependencies are heavy (8 crates with features), slowing test compilation — `crates/kirin-interpreter/Cargo.toml:17-27`
