# Interface Ergonomics Review Plan — 2026-03-12

**Scope:** Full workspace — deep focus on interface ergonomics
**Focus Areas:** Interpreter where clauses, derive attribute zoo, #[wraps]+Region limitation, PhantomData boilerplate

## Reviewer Roster

| Reviewer | Primary Theme | Deep-Dive Areas |
|----------|--------------|-----------------|
| PL Theorist | Abstractions & Type Design | Interpreter trait hierarchy (where clause verbosity), #[wraps]+Region/Block limitation |
| Physicist/DSL User | API Ergonomics & Naming | Derive attribute zoo (6+ namespaces), PhantomData boilerplate, dialect author onboarding |
| Rust Engineer | Code Quality & Idioms | All 4 pain points from implementation angle — idiomatic alternatives, derive improvements |

## Themes

1. **Abstractions & Type Design** — PL Theorist primary
2. **API Ergonomics & Naming** — Physicist primary
3. **Code Quality & Idioms** — Rust Engineer primary

## File Assignments

### All Reviewers (shared core)
- `kirin-ir/src/language.rs` — Dialect trait + 10 sub-traits
- `kirin-interpreter/src/` — Interpreter trait hierarchy
- `example/toy-lang/src/language.rs` — User-facing composition

### PL Theorist
- `kirin-interpreter/src/traits/` — ValueStore, StageAccess, BlockEvaluator decomposition
- `kirin-interpreter/src/interpret.rs` — Interpretable/CallSemantics trait definitions
- `kirin-ir/src/pipeline.rs` — StageMeta, HasStageInfo

### Physicist/DSL User
- `kirin-arith/src/lib.rs` — Simple dialect (easy path)
- `kirin-scf/src/lib.rs` — Moderate dialect (Block fields)
- `kirin-function/src/` — Complex dialect (Region, Call resolution)
- `kirin-derive-chumsky/` — HasParser derive attributes
- `kirin-derive-interpreter/` — Interpretable derive attributes

### Rust Engineer
- `kirin-derive-ir/` — Dialect derive, attribute parsing
- `kirin-arith/src/interpret_impl.rs` — Interpreter boilerplate example
- `kirin-function/src/interpret_impl.rs` — Heavy interpreter example
- `kirin-scf/src/interpret_impl.rs` — Block-handling interpreter

## Design Context (for reviewers)

Intentional patterns NOT to flag:
- `L` on method (not trait) for Interpretable/CallSemantics — breaks E0275 cycle
- `#[wraps]`/`#[callable]` as separate attributes — composability across derives
- PhantomData<T> on dialect types — needed for type parameter threading
- `active_stage()` vs `active_stage_info()` — different return types (key vs info)
- Single lifetime `HasParser<'t>` — collapsed from two-lifetime system
