# kirin-ir Review Plan — 2026-03-02

**Scope:** `crates/kirin-ir/src/` — ~4,700 lines, 38 files
**Reviewers:** PL Theorist, Compiler Engineer, Rust Engineer (Implementer), Physicist

## Themes & Assignments

| Theme | Primary | Focus Files |
|-------|---------|-------------|
| Correctness & Safety | Rust Engineer | `arena/`, `node/` (linked_list, block, ssa), `builder/context.rs`, `stage/dispatch.rs` |
| Abstractions & Type Design | PL Theorist | `language.rs`, `lattice.rs`, `stage/meta.rs`, `signature/`, `context.rs`, `detach.rs` |
| Performance & Scalability | Compiler Engineer | `arena/`, `node/linked_list.rs`, `stage/dispatch.rs`, `pipeline.rs`, `intern.rs` |
| API Ergonomics & Naming | Physicist | `builder/`, `pipeline.rs`, `node/function/`, `stage/helpers.rs`, `signature/` |
| Code Quality & Idioms | Rust Engineer | All files (general pass) |

## Module Structure

- **arena/** (6 files, ~330 lines): Generic slot-based allocator with dense/sparse hints
- **node/** (12 files, ~1,185 lines): IR structure — Statement, Block, Region, SSAValue, function hierarchy
- **stage/** (7 files, ~1,088 lines): Pipeline stage management — StageMeta, StageDispatch, StageAction
- **builder/** (4 files, ~831 lines): IR construction API — StageInfo builder, BlockBuilder, RegionBuilder
- **signature/** (3 files, ~226 lines): Function type signatures and semantics
- **query/** (2 files, ~141 lines): Structural queries
- **Core files** (~450 lines): pipeline.rs, language.rs, context.rs, lattice.rs, intern.rs, detach.rs, comptime.rs
