# kirin-ir Review Plan — 2026-03-03

**Scope:** `crates/kirin-ir/src/` — ~4,800 lines, 47 files
**Prior review:** `docs/reviews/2026-03-02-kirin-ir-review.md` (15 actioned, 18 Won't Fix)
**Focus:** Post-fix re-review — verify fixes landed correctly + find new issues introduced by the changes

## Scope

This is a follow-up review of the same crate after addressing 15 findings from the 2026-03-02 review. The review should focus on:
1. Verifying the fixes are correct and complete
2. Finding any new issues introduced by the changes
3. Reviewing areas not covered deeply in the first pass

### Module Structure

| Module | Files | Lines | Description |
|--------|-------|-------|-------------|
| `arena/` | 7 | ~350 | Arena allocator, GC, hints |
| `builder/` | 4 | ~900 | Block/region/statement builders, specialize |
| `node/` | 10 | ~1100 | IR node types (block, region, SSA, function hierarchy) |
| `stage/` | 7 | ~1000 | Stage dispatch, meta, pipeline impl |
| `signature/` | 4 | ~300 | Signature types and semantics |
| `query/` | 2 | ~140 | LinkedList/parent info traits |
| root | 7 | ~1000 | lib.rs, pipeline, context, language, lattice, intern, detach, comptime |

### Recently Changed Files (since last review)

- `arena/data.rs` — Item::new simplified, is_empty added
- `arena/gc.rs` — IdMap::get returns None, explicit return removed
- `arena/item.rs` — bon builder removed, simple constructor
- `builder/block.rs` — arg_name() added, argument_with_name deprecated
- `builder/context.rs` — specialize() param f→func
- `detach.rs` — len decrement, style fixes
- `intern.rs` — lookup uses Borrow trait
- `lib.rs` — prelude module added
- `node/function/staged.rs` — all_matching O(n^2) fix, &[T] returns
- `node/function/specialized.rs` — &[T] returns
- `node/ssa.rs` — #[doc(hidden)] on builder/test variants
- `pipeline.rs` — define_function, lookup_symbol no-alloc, link() docs

## Reviewer Roster

All four reviewers — this is a re-review with significant changes across the crate.

| Reviewer | Themes | Focus |
|----------|--------|-------|
| PL Theorist | Abstractions & Type Design | New prelude module, define_function API, all_matching correctness |
| Compiler Engineer | Performance & Scalability | Borrow-based lookup, all_matching fix, Item::new change |
| Rust Engineer | Correctness & Safety, Code Quality & Idioms | Detach len fix, IdMap::get, arg_name deprecation, new code quality |
| Physicist | API Ergonomics & Naming | define_function API, prelude contents, arg_name vs argument_with_name |

## File Assignments

| Reviewer | Primary Files | Secondary Files |
|----------|--------------|-----------------|
| PL Theorist | `lib.rs`, `pipeline.rs`, `node/function/staged.rs`, `signature/semantics.rs` | `node/function/specialized.rs`, `lattice.rs` |
| Compiler Engineer | `intern.rs`, `arena/data.rs`, `arena/gc.rs`, `arena/item.rs`, `node/function/staged.rs` | `pipeline.rs`, `arena/hint/` |
| Rust Engineer | `detach.rs`, `arena/gc.rs`, `builder/block.rs`, `builder/context.rs`, `pipeline.rs` | `arena/data.rs`, `node/ssa.rs`, `language.rs` |
| Physicist | `pipeline.rs`, `builder/block.rs`, `lib.rs` | `builder/context.rs`, `context.rs` |

## Design Context (for reviewer prompts)

The following AGENTS.md sections must be included in all reviewer prompts:

### IR Design Conventions
- **Block vs Region**: A `Block` is a single linear sequence of statements. A `Region` is a container for multiple blocks. When modeling MLIR-style operations, check whether the MLIR op uses `SingleBlock` regions — if so, use `Block` in Kirin, not `Region`.
- **`BlockInfo::terminator` is a cached pointer**: The `terminator` field in `BlockInfo` is a cached pointer to the last statement — NOT a separate statement.

### Derive Infrastructure Conventions
- **Helper attribute pattern**: `#[wraps]` and `#[callable]` are intentionally separate from `#[kirin(...)]` for composability.
- **`#[kirin(...)]` attribute convention**: Use path syntax for `crate`: `#[kirin(crate = kirin_ir)]`.

### Project Principles
- Less standalone functions is better
- Every module expects few imported names
- Use `mod.rs` for modules with multiple files
- Tests go in `kirin-test-utils` unless crate-specific

### Prior Review Won't Fix Decisions
The following were explicitly marked Won't Fix in the 2026-03-02 review — do NOT re-flag:
- Arena::Index accessing deleted items (needed for rewrite framework)
- LatticeSemantics covariant check (correct for dispatch)
- Successor/Block bidirectional conversion (intentional newtypes)
- TypeLattice as marker trait (semantic, not just alias)
- Item<T> deleted flag padding (per-item needed for rewrite)
- link_statements/link_blocks panics (catch IR corruption)
- Dialect god-trait (derive mitigates)
- SpecializedFunction as (StagedFunction, usize) (part of StagedFunction)
- BlockInfo PhantomData (GetInfo<L> dispatch)
- InternTable::intern clone (unavoidable without Rc<str>)
- DenseHint Vec<Option<T>> (simplicity trade-off)
- StatementIter locality (known trade-off)
- Test manually implements Dialect (avoids derive dependency)

## Themes

All five themes apply:
1. **Correctness & Safety** — verify P0 fixes, check new code
2. **Abstractions & Type Design** — prelude contents, define_function API design
3. **Performance & Scalability** — verify all_matching fix, Borrow lookup
4. **API Ergonomics & Naming** — define_function, arg_name, prelude
5. **Code Quality & Idioms** — new code follows Rust idioms
