# Plan 3: Parser Two-Pass & Ergonomics

**Crates**: `kirin-chumsky`, `kirin-chumsky-derive`, `kirin-chumsky-format`
**Review source**: parser-critic, parser-simplifier

## Goal

Fix the block forward-reference correctness bug, simplify the lifetime surface of parser traits, and improve ergonomics for dialect parser authors.

## Changes

### Phase 1: Correctness (P0)

1. **Fix block forward-reference ordering** (`ast.rs:388`)
   - Region emission registers blocks AFTER emitting bodies
   - Forward `br ^bbN` panics if `^bbN` hasn't been emitted yet
   - **Fix**: Two-pass emit — first pass registers all block names, second pass emits bodies
   - Mirrors the pipeline parser's own two-pass design
   - Add regression test for forward branch references

### Phase 2: Performance (P1)

2. **Replace `std::HashMap` with `FxHashMap`** in `EmitContext` (`traits.rs:261-262`) and `parse_text.rs`
   - Add `rustc-hash` dependency to kirin-chumsky

3. **Eliminate `tokens.to_vec()` in `parse_one_declaration`** (`syntax.rs:136`)
   - O(N) allocation per declaration, O(N^2) total for pipeline parsing
   - Use slice or iterator instead

### Phase 3: Lifetime Simplification (P1)

4. **Collapse `HasParser<'tokens, 'src>` to `HasParser<'src>`**
   - The two lifetimes are almost always unified at call sites via `for<'src> L: HasParser<'src, 'src>`
   - Tie token lifetime to source lifetime (they already are in practice)
   - This is a pervasive change affecting all parser traits and derive output
   - **Approach**:
     - Update `HasParser` trait definition
     - Update `HasDialectParser` accordingly
     - Update `EmitIR` trait bounds
     - Update kirin-chumsky-derive code generation
     - Update all dialect `HasParser` impls
   - **Risk**: High — touches many files. Needs careful migration.

### Phase 4: Ergonomics (P2)

5. **Add `ParseDialect<L>` helper trait** to bundle HRTB bounds
   - Blanket-implemented supertrait that bundles `for<'src> L: HasParser<'src> + HasDialectParser<'src> + ...`
   - Pure ergonomic improvement — no behavior change
   - Dialect authors write `L: ParseDialect` instead of 4-line where clauses

6. **Shared `RecursiveAST<T>`** replaces per-dialect `ASTSelf`
   - Currently each dialect generates its own `FooASTSelf` recursive wrapper
   - Extract a generic `RecursiveAST<T>` in kirin-chumsky
   - Derive generates `type ASTSelf = RecursiveAST<FooAST<...>>`

7. **Rename generated AST types** to `__FooAST` / `__FooASTSelf` with `#[doc(hidden)]`
   - Decision from interview: safe to rename, no code references them directly

### Phase 5: Derive Cleanup (P2-P3)

8. **Unify `input_requires_ir_type`** (`input.rs:42-61` and `input.rs:90-111`)
   - Single generic function parameterized by layout type

9. **Remove or use `_ir_path` in `BoundsBuilder`** (`bounds.rs:17`)
   - Dead parameter

## Files Touched

- `crates/kirin-chumsky/Cargo.toml` (add rustc-hash)
- `crates/kirin-chumsky/src/ast.rs` (two-pass emit)
- `crates/kirin-chumsky/src/traits.rs` (HasParser, EmitContext)
- `crates/kirin-chumsky/src/syntax.rs` (tokens.to_vec)
- `crates/kirin-chumsky/src/parse_text.rs`
- `crates/kirin-chumsky-derive/src/input.rs`
- `crates/kirin-chumsky-format/src/bounds.rs`
- All dialect crates' parser impls (for HasParser lifetime change)

## Validation

```bash
cargo nextest run -p kirin-chumsky
cargo nextest run -p kirin-chumsky-derive
cargo nextest run --workspace  # all dialects use parser traits
cargo test --doc -p kirin-chumsky
```

## Recommended Skills & Workflow

**Setup**: `/using-git-worktrees` — isolate in a worktree off `main`

**Phase 1 (Correctness)**: Bug fix with regression test.
- `/systematic-debugging` to reproduce the forward-reference panic, trace the emit path in `ast.rs`
- `/test-driven-development` — write a test with forward `br ^bbN` reference that panics, then implement two-pass emit to fix it

**Phase 2 (Performance)**: Mechanical changes.
- `/simplify` after FxHashMap swap and tokens.to_vec elimination

**Phase 3 (Lifetime Simplification)**: High-risk, pervasive change — needs the most careful planning.
- `/brainstorming` before `HasParser` lifetime collapse — this is the highest-risk item across all 6 plans. Explore: which call sites actually need two distinct lifetimes? What breaks when unified? Can we prototype on a single dialect first?
- `/kirin-rfc-writer` — this change is significant enough to warrant an RFC documenting the before/after trait signatures, migration path, and rationale
- `/test-driven-development` — ensure all existing parser tests pass after the change
- `/subagent-driven-development` — parallelize dialect-by-dialect migration of HasParser impls

**Phase 4 (Ergonomics)**: New API surface.
- `/brainstorming` for `ParseDialect<L>` trait design and `RecursiveAST<T>` — explore bound bundling strategy, ensure it composes with existing `ParseStatementTextExt` pattern
- `/test-driven-development` for each new trait/type

**Completion**:
- `/verification-before-completion` — full workspace tests (all dialects use parser traits)
- `/simplify` — final pass on changed files
- `/requesting-code-review` — this plan has the highest risk; thorough review essential
- `/finishing-a-development-branch`

## Non-Goals

- Merging `HasDialectParser` + `HasParser` into one trait (medium risk, deferred)
- Reducing AST type params from 4 to 2 (couples with lifetime change, deferred)
- Changing EmitIR fallibility (two-pass emit handles the immediate problem)
