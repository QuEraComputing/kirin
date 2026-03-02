# Plan 5: Pretty Printer Simplification

**Crates**: `kirin-prettyless`, `kirin-prettyless-derive`
**Review source**: printer-lexer-critic, printer-lexer-simplifier

## Goal

Reduce the pretty printer's trait surface from 5 traits to 2 (`PrettyPrint` + `RenderStage`), replace the 12-method `PrettyPrintExt` with a builder pattern, and remove dead code.

## Changes

### Phase 1: Dead Code Removal (P3)

1. **Remove `Config.line_numbers`**
   - Decision from interview: dead code. Set and tested but never read by the rendering path.
   - Bat pager hardcodes its own `line_numbers(true)`
   - Remove field, update constructors, remove tests that only test this field

2. **Remove dead alignment code** (`result_width`, `max_result_width`)
   - ~20 lines of unused width tracking

### Phase 2: Builder Pattern (P1, ~100 lines saved)

3. **Replace `PrettyPrintExt` 12-method family with `RenderBuilder`**
   - Current: `sprint()`, `sprint_with_globals()`, `sprint_with_config()`, `sprint_with_config_and_globals()`, `print()`, `print_with_globals()`, ... (×2 for pipeline variants)
   - Target:
     ```rust
     statement.render(&stage)
         .config(config)        // optional
         .globals(&gs)          // optional
         .to_string();          // or .print(), .bat()
     ```
   - Implement `RenderBuilder` struct with optional fields + terminal methods
   - Keep `sprint()` as a convenience shorthand (delegates to builder with defaults)

### Phase 3: Trait Consolidation (P2, 3 traits removed)

4. **Merge `ScanResultWidth` into `PrettyPrint`**
   - `ScanResultWidth` is a pre-pass that mutates `&mut Document`
   - Make it a default method on `PrettyPrint` (or call it automatically in the builder)
   - 1 trait removed, 8 impls become default methods

5. **Remove `PrettyPrintName` and `PrettyPrintType` as separate traits**
   - These don't require `L: PrettyPrint` (inconsistent with rest of framework)
   - Fold into `PrettyPrint` as associated methods or default impls
   - 2 traits removed, 8 impls consolidated

6. **Collapse `PrintExt` / `PipelinePrintExt` method explosion**
   - After builder pattern is in place, these become thin wrappers
   - ~10 methods consolidated

### Phase 4: Documentation (P2)

7. **Document the roundtrip property** on `PrettyPrint` trait
   - Add trait-level doc explaining that `parse(sprint(ir)) == ir` is the target invariant

8. **Improve `Document` API documentation** for manual `PrettyPrint` implementors

## Files Touched

- `crates/kirin-prettyless/src/lib.rs` (trait definitions)
- `crates/kirin-prettyless/src/config.rs` (remove line_numbers)
- `crates/kirin-prettyless/src/ext.rs` (PrettyPrintExt → RenderBuilder)
- `crates/kirin-prettyless/src/scan.rs` (merge ScanResultWidth)
- `crates/kirin-prettyless/src/document.rs` (docs)
- `crates/kirin-prettyless-derive/` (update generated code for merged traits)
- All dialect crates' PrettyPrint impls (if trait surface changes)

## Validation

```bash
cargo nextest run -p kirin-prettyless
cargo nextest run --workspace  # all dialects implement PrettyPrint
cargo test --doc -p kirin-prettyless
cargo insta review  # snapshot tests may change formatting
```

## Recommended Skills & Workflow

**Setup**: `/using-git-worktrees` — isolate in a worktree off `main`

**Phase 1 (Dead Code Removal)**: Low-risk mechanical cleanup.
- `/simplify` after removing `Config.line_numbers` and dead alignment code

**Phase 2 (Builder Pattern)**: New API design — needs creative exploration.
- `/brainstorming` before designing `RenderBuilder` — explore builder API shape, method naming (`.to_string()` vs `.sprint()` vs `.render()`), whether `bon::bon` pattern should be used for consistency with rest of codebase, backward compatibility strategy (keep `sprint()` as shorthand?)
- `/test-driven-development` — write tests using the new builder API, then implement the builder
- `/simplify` after replacing the 12-method family

**Phase 3 (Trait Consolidation)**: Significant API surface change.
- `/brainstorming` before merging traits — explore whether `ScanResultWidth` should be a default method or called automatically by the builder. Explore how `PrettyPrintName`/`PrettyPrintType` fold into `PrettyPrint` without requiring `L: PrettyPrint` bounds (associated methods vs default impls vs separate module)
- `/test-driven-development` — ensure all existing pretty-print tests pass after consolidation
- `/subagent-driven-development` — parallelize: one agent handles ScanResultWidth merge, another handles PrettyPrintName/Type consolidation

**Phase 4 (Documentation)**: Straightforward.
- No special skill needed — just write docs

**Completion**:
- `/verification-before-completion` — full workspace tests + `cargo insta review` for snapshot changes
- `/requesting-code-review` — builder pattern and trait consolidation need review
- `/finishing-a-development-branch`

## Non-Goals

- Inlining kirin-lexer into kirin-chumsky (decision: keep separate)
- Changing the Document algebra
- Adding `try_sprint()` variants (panics are fine for diagnostic tool)
- `PipelineDocument` arena reuse (not a hot path)
