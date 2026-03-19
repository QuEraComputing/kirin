# Add `#[diagnostic::on_unimplemented]` to Key Traits

## Problem

When users hit trait bound failures involving `HasStageInfo`, `Dialect`, `StageMeta`, `Interpreter`, or `CallSemantics`, the compiler emits generic "trait not satisfied" errors with no guidance on how to fix them. This is especially painful when the errors arise from derive-generated code with mangled names like `__InterpI` or `__InterpL`.

The existing `AsBuildStage` trait at `crates/kirin-ir/src/builder/stage_info.rs:17-20` demonstrates the pattern well:

```rust
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a builder stage -- cannot construct IR on finalized `StageInfo`",
    note = "use `stage.with_builder(|b| {{ ... }})` to get a `&mut BuilderStageInfo` for construction"
)]
pub trait AsBuildStage<L: Dialect> { ... }
```

This produces clear, actionable errors. The same pattern should be applied to the most commonly misused traits.

## Research Findings

### Target traits and their locations

**In `kirin-ir`:**

1. **`HasStageInfo<L>`** at `crates/kirin-ir/src/stage/meta.rs:28`
   - Common error: stage enum does not contain a `StageInfo` variant for dialect `L`.
   - Typical fix: add a variant wrapping `StageInfo<L>` to the stage enum, or check that `#[derive(StageMeta)]` includes the dialect.

2. **`Dialect`** at `crates/kirin-ir/src/language.rs:103`
   - Common error: a type is used as a dialect but does not implement the 19+ supertrait bounds.
   - Typical fix: use `#[derive(Dialect)]` on the enum/struct.

3. **`StageMeta`** at `crates/kirin-ir/src/stage/meta.rs:68`
   - Common error: a stage type is passed to `Pipeline::add_stage` without implementing `StageMeta`.
   - Typical fix: use `#[derive(StageMeta)]` on the stage enum.

**In `kirin-interpreter`:**

4. **`Interpreter<'ir>`** at `crates/kirin-interpreter/src/interpreter.rs:14`
   - Common error: a custom interpreter type does not implement `BlockEvaluator` (and its sub-traits).
   - This trait is a blanket impl over `BlockEvaluator`, so the real fix is implementing the sub-traits.

5. **`CallSemantics<'ir, I>`** at `crates/kirin-interpreter/src/call.rs:12`
   - Common error: a dialect type does not have call semantics for the interpreter.
   - Typical fix: implement `SSACFGRegion` for standard function bodies, or implement `CallSemantics` directly.

### Existing diagnostic

`AsBuildStage` is the only trait with `#[diagnostic::on_unimplemented]` in the workspace. The review report (P2-E) specifically calls out the missing diagnostics on the 5 traits above.

### Derive-generated bounds context

From the review: derive macros emit bounds like `__InterpI: Interpreter<'__ir>` and `__CallSemI::Error: From<InterpreterError>`. When these bounds are unsatisfied, the error points to generated code with mangled names. The `#[diagnostic::on_unimplemented]` on the target traits would replace the opaque "trait bound not satisfied" message with a human-readable explanation.

## Proposed Design

### `HasStageInfo<L>`

```rust
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not contain a `StageInfo<{L}>` variant",
    note = "add a variant wrapping `StageInfo<{L}>` to your stage enum, or check your `#[derive(StageMeta)]`"
)]
pub trait HasStageInfo<L: Dialect> { ... }
```

### `Dialect`

```rust
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `Dialect`",
    note = "use `#[derive(Dialect)]` to generate the required IR accessor trait implementations"
)]
pub trait Dialect: ... { ... }
```

### `StageMeta`

```rust
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `StageMeta`",
    note = "use `#[derive(StageMeta)]` on your stage enum to generate stage identity methods"
)]
pub trait StageMeta: Sized { ... }
```

### `Interpreter<'ir>`

```rust
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `Interpreter`",
    note = "implement `ValueStore`, `StageAccess`, and `BlockEvaluator` on your interpreter type, or use `StackInterpreter`/`AbstractInterpreter`"
)]
pub trait Interpreter<'ir>: BlockEvaluator<'ir> {}
```

### `CallSemantics<'ir, I>`

```rust
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `CallSemantics` for this interpreter",
    note = "implement `SSACFGRegion` for standard function body evaluation, or implement `CallSemantics` directly for custom call semantics"
)]
pub trait CallSemantics<'ir, I: Interpreter<'ir>>: Dialect { ... }
```

### Message guidelines

Following the `AsBuildStage` pattern:
- `message` describes WHAT is wrong in terms of the user's types.
- `note` describes HOW to fix it with specific derive macros or trait implementations.
- Use `{Self}` and `{L}` / `{I}` placeholders where available.
- Keep messages concise -- one sentence each.

## Implementation Steps

1. Add `#[diagnostic::on_unimplemented]` to `HasStageInfo<L>` in `crates/kirin-ir/src/stage/meta.rs`.
2. Add `#[diagnostic::on_unimplemented]` to `Dialect` in `crates/kirin-ir/src/language.rs`.
3. Add `#[diagnostic::on_unimplemented]` to `StageMeta` in `crates/kirin-ir/src/stage/meta.rs`.
4. Add `#[diagnostic::on_unimplemented]` to `Interpreter` in `crates/kirin-interpreter/src/interpreter.rs`.
5. Add `#[diagnostic::on_unimplemented]` to `CallSemantics` in `crates/kirin-interpreter/src/call.rs`.
6. Verify with `cargo build --workspace` that the attributes are accepted (they are stable since Rust 1.78).
7. Optionally add a compile-fail test (using `trybuild`) that verifies the diagnostic message appears. The existing `AsBuildStage` test at `crates/kirin-ir/tests/trybuild/` can serve as a template.

## Risk Assessment

**Very low risk.** `#[diagnostic::on_unimplemented]` is purely additive -- it only affects error messages, not compilation or runtime behavior. The attribute is a hint to the compiler; if malformed, it is silently ignored (per the diagnostic namespace spec).

The only consideration is that `{L}` and `{I}` type parameter placeholders in the message may display fully-qualified type names (e.g., `my_crate::MyDialect` instead of `MyDialect`). This is acceptable and consistent with standard compiler output.

## Testing Strategy

- **Compile-fail tests with `trybuild`:** Add tests for each trait that trigger the `on_unimplemented` message and verify the expected error output. Follow the pattern in `crates/kirin-ir/tests/trybuild/` for `AsBuildStage`.
- **Manual verification:** Temporarily introduce a trait bound failure in test code and inspect the compiler output.
- `cargo build --workspace` and `cargo nextest run --workspace` to confirm no regressions.
