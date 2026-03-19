# Pipeline Convenience Methods: `stage_by_name()` and `resolve_function()`

## Problem

Using the Pipeline API to look up a stage by name and resolve a function to a `SpecializedFunction` requires a multi-step ceremony. The current pattern in `example/toy-lang/src/main.rs:76-170` demonstrates this:

1. **Stage lookup** (lines 76-88): iterate `pipeline.stages()`, check each stage's `stage_name()`, resolve the `GlobalSymbol` to a string, compare, extract `stage_id()`. This is 13 lines for a conceptually simple operation.

2. **Function resolution** (lines 90-110): `lookup_symbol` -> `function_by_name` -> `function_info` -> `staged_functions().get(&stage_id)`. Four fallible steps, each needing its own `.ok_or_else(...)`.

3. **Specialization resolution** (lines 150-175, `resolve_specialization`): requires a dialect-generic helper function that accesses `pipeline.stage()` -> `try_stage_info()` -> `staged_function.get_info(stage_info)` -> filter specializations.

Every user of Pipeline must reinvent these chains. The error messages are ad-hoc (anyhow strings), not structured.

## Research Findings

### Current Pipeline API (`crates/kirin-ir/src/pipeline.rs`)

The Pipeline struct provides low-level building blocks:
- `stages() -> &[S]` -- raw slice, no name-based lookup
- `stage(id: CompileStage) -> Option<&S>` -- by numeric ID only
- `lookup_symbol(name: &str) -> Option<GlobalSymbol>` -- string to symbol
- `function_by_name(sym: GlobalSymbol) -> Option<Function>` -- symbol to function
- `function_info(func: Function) -> Option<&Item<FunctionInfo>>` -- function to info
- `resolve(sym: GlobalSymbol) -> Option<&str>` -- symbol back to string

The `StageMeta` trait provides `stage_name() -> Option<GlobalSymbol>` and `stage_id() -> Option<CompileStage>`.

### Stage Lookup Chain

To find a stage by name:
```
pipeline.stages().iter()
    .find_map(|s| {
        let sym = s.stage_name()?;
        let name = pipeline.resolve(sym)?;
        if name == target_name { s.stage_id() } else { None }
    })
```

This requires `S: StageMeta` and access to the pipeline's symbol table for resolution.

### Function Resolution Chain

To get a `SpecializedFunction` for a named function at a given stage:
```
lookup_symbol(name) -> function_by_name(sym) -> function_info(func)
    -> staged_functions().get(&stage_id) -> get_info(stage_info)
    -> specializations().first()
```

This requires both the Pipeline and a `&StageInfo<L>` reference.

## Proposed Design

### `Pipeline::stage_by_name`

```rust
impl<S: StageMeta> Pipeline<S> {
    /// Look up a stage by its human-readable name, returning its `CompileStage`.
    ///
    /// Returns `None` if no stage has the given name.
    pub fn stage_by_name(&self, name: &str) -> Option<CompileStage> {
        self.stages.iter().find_map(|s| {
            let sym = s.stage_name()?;
            let resolved = self.global_symbols.resolve(sym)?;
            if resolved.as_str() == name {
                s.stage_id()
            } else {
                None
            }
        })
    }
}
```

This is a simple convenience wrapper. No new error types needed -- `Option` suffices since the caller can contextualize the failure.

### `Pipeline::resolve_function`

```rust
impl<S> Pipeline<S> {
    /// Resolve a named function at a given stage to its `StagedFunction`.
    ///
    /// Combines `lookup_symbol` -> `function_by_name` -> `function_info`
    /// -> staged function lookup into a single call.
    ///
    /// Returns `None` if any step in the chain fails.
    pub fn resolve_staged_function(
        &self,
        func_name: &str,
        stage: CompileStage,
    ) -> Option<StagedFunction> {
        let sym = self.lookup_symbol(func_name)?;
        let func = self.function_by_name(sym)?;
        let info = self.function_info(func)?;
        info.staged_functions().get(&stage).copied()
    }
}
```

This collapses the 4-step chain into one call. Returns `Option` for consistency with the existing API style.

### Not proposed: `resolve_specialization`

Specialization resolution requires `&StageInfo<L>` and dialect-specific info access (`StagedFunction::get_info`). Adding this to Pipeline would require `S: HasStageInfo<L>` and a dialect type parameter, making it less general. Users who need specialization resolution can compose `resolve_staged_function` with their own dialect-specific logic. If demand warrants it, a future `Staged` convenience method could be added.

## Implementation Steps

1. Add `stage_by_name` to `impl<S: StageMeta> Pipeline<S>` -- place it in the existing non-`bon` impl block (near line 34).
2. Add `resolve_staged_function` to `impl<S> Pipeline<S>` -- no trait bounds needed.
3. Update `example/toy-lang/src/main.rs` to use the new methods, demonstrating the reduced ceremony.
4. Add unit tests in the existing `mod tests` block in `pipeline.rs`.

## Risk Assessment

**Low risk.** These are additive convenience methods with no behavior changes. They compose existing public methods and return `Option`, matching the crate's convention. No new dependencies or trait bounds on the struct.

One consideration: `stage_by_name` is O(N) in the number of stages. This is fine since pipelines typically have fewer than 10 stages. If this ever matters, a `name_to_stage` index could be added later.

## Testing Strategy

- Unit test `stage_by_name` with named and unnamed stages, verifying `Some`/`None` behavior.
- Unit test `resolve_staged_function` by constructing a pipeline with a named function at a stage, verifying the full chain returns the correct `StagedFunction`.
- Test `resolve_staged_function` returns `None` for missing function name, missing stage, and unlinked staged function.
- Update `toy-lang` to use the new methods and verify all existing toy-lang tests still pass.
