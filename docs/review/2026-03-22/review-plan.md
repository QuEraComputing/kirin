# Full Workspace Review Plan — 2026-03-22

**Scope:** Full workspace — 24 crates (~52,700 lines)
**Previous review:** 2026-03-18 (8 P1, 16 P2, 20 P3 — many addressed since then)
**Changes since last review:** 286 files, ~10K insertions / ~4K deletions
**Major changes:** Graph unification (DiGraph/UnGraph → GraphInfo), Signature field migration, EmitIR builder reuse, clippy fixes

## Reviewer Roster (6 reviewers)

| Role | Persona | Focus |
|------|---------|-------|
| Formalism | PL Theorist | Abstraction composability, literature alignment, semantic ambiguity |
| Code Quality | Implementer (review mode) | Clippy workarounds, duplication, Rust best practices |
| Ergonomics/DX | Physicist | User repetition, lifetime complexity, concept budget |
| Soundness Adversary | Invariant Breaker | Arena/ID safety, builder bypass, interpreter trust model |
| Dialect Author | Domain Consumer | Framework interaction, domain alignment, error paths |
| Compiler Engineer | Infra Pragmatist | Compile time, error messages, build graph, scalability |

## Review Units

| Unit | Crates | Lines | Reviewers |
|------|--------|-------|-----------|
| U1: Core IR | kirin-ir | 9,177 | All 6 |
| U2: Parser Runtime | kirin-chumsky | 6,399 | All 6 |
| U3: Derive Infra | kirin-derive-toolkit, kirin-derive-ir, kirin-derive-chumsky, kirin-derive-prettyless, kirin-derive-interpreter | 16,172 | All 6 |
| U4: Interpreter | kirin-interpreter | 6,933 | All 6 |
| U5: Printer | kirin-prettyless | 3,399 | All 6 |
| U6: Dialects | kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-cmp, kirin-function | 3,233 | All 6 |
| U7: Testing+Examples | kirin-test-*, toy-lang, toy-qc, kirin-lexer, kirin-interval | ~5,400 | Code Quality, Ergonomics |
| U8: Integration Tests | tests/ | 1,955 | Code Quality |

## Design Context (included in all reviewer prompts)

Relevant AGENTS.md sections:
- Derive Infrastructure Conventions
- IR Design Conventions
- Interpreter Conventions
- Chumsky Parser Conventions
- Test Conventions
- Dialect Domain Context table
