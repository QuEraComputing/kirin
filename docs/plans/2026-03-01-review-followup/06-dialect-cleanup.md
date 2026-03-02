# Plan 6: Dialect Cleanup

**Crates**: `kirin-cf`, `kirin-scf`, `kirin-arith`, `kirin-function`, `kirin-constant`
**Review source**: dialect-critic, dialect-simplifier

## Goal

Fix correctness hazards in dialect implementations, remove duplicated operations, and improve documentation quality to match kirin-constant's exemplary standard.

## Changes

### Phase 1: Correctness (P0)

1. **Address Div/Rem panic on division by zero** (`kirin-arith`)
   - Currently panics on divide-by-zero in interpret impl
   - Options: (a) use checked operations returning error, (b) document the contract
   - Recommended: Return `InterpreterError::custom("division by zero")` for concrete interpreter, propagate abstract domain's handling for abstract interpreter

### Phase 2: Deduplication (P1)

2. **Remove `Return` from `kirin-cf`**
   - Decision from interview: `kirin-function::Return` is canonical
   - Remove `ControlFlow::Return` variant from kirin-cf
   - Update all code referencing `kirin_cf::Return` to use `kirin_function::Return`
   - May need to add kirin-function as dependency where kirin-cf was used for Return
   - **Risk**: Low-medium — need to check all downstream usages

### Phase 3: Stage Resolution Adoption

3. **Adopt stage resolution helper** (after Plan 2 lands)
   - Replace 8-line stage resolution chains in all dialect interpret impls
   - ~15 lines saved per dialect (5+ dialects = ~75 lines total)

4. **Adopt `Pipeline::function_by_name()`** (after Plan 1 lands)
   - Replace O(N) linear scan in `Call::interpret` with O(1) lookup
   - Simplifies kirin-function's interpret impl by ~30 lines

### Phase 4: Documentation (P2)

5. **Add module-level docs to `kirin-cf` and `kirin-scf`**
   - Match kirin-arith and kirin-constant quality
   - Document each operation's semantics, MLIR correspondence, and properties
   - Document the `Lexical` vs `Lifted` distinction in kirin-function

6. **Document E0275 limitation** for Region-containing types with `#[wraps]` + `HasParser`
   - Add to Lambda type docs in kirin-function
   - Add note to AGENTS.md Chumsky Parser Conventions

### Phase 5: Consistency (P2)

7. **Standardize dialect import patterns**
   - Inconsistent patterns across dialects (some use prelude, some use direct imports)
   - Establish and apply consistent pattern

8. **Use `#[derive(Interpretable)]` for wrapper enums**
   - e.g. `StructuredControlFlow` in kirin-scf
   - Low effort, removes ~20 lines per wrapper

## Files Touched

- `crates/kirin-cf/src/lib.rs` (remove Return variant)
- `crates/kirin-arith/src/interpret_impl.rs` (div/rem error handling)
- `crates/kirin-cf/src/lib.rs` (module docs)
- `crates/kirin-scf/src/lib.rs` (module docs)
- `crates/kirin-function/src/lambda.rs` (E0275 docs)
- All dialect interpret impls (stage resolution, standardized imports)
- `AGENTS.md` (E0275 note)

## Validation

```bash
cargo nextest run -p kirin-cf
cargo nextest run -p kirin-scf
cargo nextest run -p kirin-arith
cargo nextest run -p kirin-function
cargo nextest run --workspace
cargo test --doc --workspace
```

## Dependencies

- Phase 3 depends on Plan 2 (interpreter dispatch) for stage resolution helper
- Phase 3 depends on Plan 1 (IR core) for function_by_name
- Phases 1, 2, 4, 5 are independent and can proceed immediately

## Recommended Skills & Workflow

**Setup**: `/using-git-worktrees` — isolate in a worktree off `main`

**Phase 1 (Correctness)**: Bug fix.
- `/systematic-debugging` to confirm the div/rem panic is reachable and understand current behavior
- `/test-driven-development` — write a test that triggers division by zero, assert it returns `InterpreterError` not panic, then fix

**Phase 2 (Return Deduplication)**: Cross-crate refactor.
- `/brainstorming` before removing `Return` from kirin-cf — explore all downstream usages, whether kirin-cf should depend on kirin-function or if callers should switch imports
- `/subagent-driven-development` — one agent searches for all `kirin_cf::Return` usages, another prepares the kirin-function dependency additions
- `/test-driven-development` — ensure existing tests pass after the switch

**Phase 3 (Stage Resolution Adoption)**: Depends on Plan 2 landing.
- `/executing-plans` — once Plan 2's `resolve_stage` helper exists, mechanically adopt it across all dialects
- `/simplify` after adoption

**Phase 4-5 (Documentation & Consistency)**: Moderate effort.
- `/brainstorming` for standardized dialect import patterns — survey current patterns across all dialects, propose a canonical pattern
- `/subagent-driven-development` — parallelize: one agent writes kirin-cf docs, another writes kirin-scf docs, another handles import standardization
- `/simplify` after each dialect is updated

**Completion**:
- `/verification-before-completion` — full workspace tests
- `/requesting-code-review`
- `/finishing-a-development-branch`

## Non-Goals

- ArithValue match-arm repetition (~150 lines) — mechanical and greppable, macro would hurt IDE
- `Return` overlap documentation (removed instead)
- PhantomData boilerplate — tracked in Plan 4 (derive infrastructure)
- Attribute-driven interpretation (`#[kirin(interpret = "binary_op")]`) — high effort, deferred
