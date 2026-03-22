# U2: Parser Runtime -- Soundness Review

## Invariant Inventory

| Invariant | Location | Enforcement |
|-----------|----------|-------------|
| Forward-ref SSAs resolved before finalize | emit_ir.rs:129 creates Unresolved | Builder-enforced (finalize checks) |
| SSA names unique per scope | emit_ir.rs:92 register_ssa | Not enforced (last-write-wins HashMap) |
| Block names unique per scope | emit_ir.rs:99 register_block | Not enforced (last-write-wins HashMap) |
| Tokens non-empty for pipeline parse | parse_text.rs:326 | Runtime-always (Result) |
| Forward progress during parse loop | parse_text.rs:387 ensure_forward_progress | Runtime-always (Result) |
| Stage exists after resolve_or_create | parse_text.rs:365 | Runtime-always (Result) |
| Pipeline link succeeds | parse_text.rs:377 | Runtime-always (expect) |
| Function symbol exists for stage decl | parse_text.rs:229,232 | Runtime-always (expect) |

## Findings

### [P1] [confirmed] Duplicate SSA names silently shadow earlier definitions -- emit_ir.rs:91-93

**Invariant:** Each SSA name within a function body should map to exactly one SSAValue.

**Enforcement:** Not enforced. `register_ssa` uses `HashMap::insert`, which silently overwrites the previous mapping.

**Attack:** Parse input containing two statements defining `%x`:
```
%x = add %a, %b;
%x = mul %c, %d;
%y = use %x;
```
The second `%x` silently shadows the first. `%y` will reference the second `%x`. The first `%x`'s SSAValue becomes an orphan with no uses, but the IR contains it as a live result with no semantic error.

**Consequence:** Silent semantic corruption -- SSA single-assignment invariant violated without error. The IR contains two definitions of `%x` but only the second is reachable by name.

**Reachability:** Normal use -- any duplicated SSA name in text input.

**Suggested mitigation:** Check for existing entry before insert; return `EmitError::DuplicateSSA` on collision.

### [P2] [confirmed] `expect` panics in pipeline parse on valid-looking input -- parse_text.rs:377,432

**Invariant:** `Pipeline::link` should not fail for functions created by the parser.

**Enforcement:** Runtime-always (`expect`). However, `link` returns `Result<(), PipelineError>` and the error variant `UnknownFunction` can occur if there is a logic error in the two-pass system.

**Attack:** Not easily triggered through normal text input since the parser creates the functions itself. However, if `link` is called with a function ID that was somehow invalidated between pass 1 and pass 2 (e.g., concurrent mutation of the pipeline), the `expect` would panic rather than returning a parse error.

**Consequence:** Panic instead of `FunctionParseError`.

**Reachability:** Adversarial (requires concurrent mutation or internal logic bug).

**Suggested mitigation:** Replace `expect` with `map_err` converting to `FunctionParseError`.

### [P2] [confirmed] `expect` on first-pass function/symbol resolution -- parse_text.rs:229,232

**Invariant:** Stage declarations should always have a resolved function and function symbol.

**Enforcement:** Runtime-always (`expect`). The code sets `function = Some(...)` only for `DeclKeyword::Stage`, then dispatches generically. If the dispatch path for a `Stage` keyword somehow receives `None` (internal logic mismatch), the `expect` panics.

**Attack:** Currently unreachable through normal input because the conditional at line 346 ensures these are `Some` for stage declarations. But refactoring could break this invariant.

**Consequence:** Panic instead of error return.

**Reachability:** Adversarial (internal logic error only).

**Suggested mitigation:** Use `ok_or_else` with a `FunctionParseError` for defense-in-depth.

### [P3] [likely] Duplicate block names silently shadow -- emit_ir.rs:99-101

**Invariant:** Block names should be unique within a function.

**Enforcement:** Not enforced. Same `HashMap::insert` pattern as SSA names.

**Attack:** Parse input with two blocks named `^entry`. Second registration overwrites the first. Any branch targeting `^entry` reaches the second block; the first block becomes unreachable.

**Consequence:** Silent control flow corruption -- unreachable blocks in IR.

**Reachability:** Normal use with duplicated block names.

**Suggested mitigation:** Check for existing entry; return error on collision.

## Strengths

- Two-pass parsing architecture ensures all staged-function headers are visible before specialization bodies are emitted, preventing forward-reference issues.
- Forward progress check (`ensure_forward_progress`) prevents infinite loops on parse failure.
- Error types are rich and structured (`FunctionParseError` with kind, span, message, source chain).
- Relaxed dominance mode for graph bodies is cleanly scoped through `set_relaxed_dominance` rather than global state.
- All chumsky parser combinators are safe by construction -- malformed input produces parse errors, not panics.
