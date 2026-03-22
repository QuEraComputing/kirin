# Implementation Notes

Issues, design decisions, and limitations discovered during the 2026-03-22 full workspace refactor.

---

## 1. CF Roundtrip Mismatch (pre-existing, not fixed)

**Crate:** kirin-cf, kirin-ir
**Severity:** design gap

`Successor::Display` (in `kirin-ir/src/node/block.rs:45-48`) outputs raw arena IDs (`^0`, `^1`), while block headers in the pretty printer resolve names through the symbol table (`^entry`, `^exit`). This creates a roundtrip mismatch: parsing `br ^exit(%x)` then printing produces `br ^0(%x)`.

**Impact:** CF tests cannot use full roundtrip assertions. Wave 6 kept them as parse-only tests with structural assertions (block count, terminator presence).

**Fix required:** `Successor`'s pretty printing needs access to the stage's symbol table to resolve IDs back to names. This is a deeper change — `Display` doesn't carry context, so it would need to go through `PrettyPrint` with an `IRRenderCtx` instead.

---

## 2. `Vec<ResultValue>` Not Supported by Derive Macros (design constraint)

**Crate:** kirin-derive-chumsky
**Severity:** design constraint
**Discovered in:** Wave 5a (SCF result values)

The derive macros explicitly reject `Vec<ResultValue>` fields with the error: *"ResultValue field cannot be a Vec, consider implementing the builder manually."* This prevented the original plan of giving `For` multiple result values (`results: Vec<ResultValue>`) to match MLIR's multi-result `scf.for`.

**Workaround applied:** Used a single `result: ResultValue` for both `If` and `For`. This works because `Continuation::Yield(V)` carries a single value. Programs needing multiple loop-carried values would need to pack them into a single value type (e.g., a tuple/struct).

**Implications:** Kirin's `scf.for` is less expressive than MLIR's — it supports one loop-carried accumulator, not N. This is adequate for current use cases but would need addressing for full MLIR parity.

---

## 3. SCF `If` Always Requires a Result Type (design change)

**Crate:** kirin-scf
**Severity:** breaking change
**Discovered in:** Wave 5a

Adding `result: ResultValue` to `If` means the parser now always expects `-> <type>` in the text format. The existing toy-lang programs (`factorial.kirin`, `branching.kirin`) used `if` in a "void" context where branches terminated with `ret` instead of `yield`.

**Fix applied:** Updated toy-lang programs to use `yield` inside `if` branches (matching MLIR's `scf.if` convention where branches must terminate with `scf.yield`). `ret` inside an SCF block was incorrect per MLIR semantics — it should propagate control up to the function level, but `eval_block` only exits on `Yield`.

**Design note:** MLIR's `scf.if` supports zero results (void if). Kirin's current `If` always produces one result. Supporting void `if` would require making `result` optional or using a sentinel type.

**Design exploration (2026-03-22):** Attempted to implement void-if support. Three approaches were investigated:

1. **`Option<ResultValue>` (rejected):** The derive macros explicitly reject `Option<ResultValue>` in both the builder template (`kirin-derive-toolkit/src/template/builder_template/helpers.rs:288-295, 432-439`) and the format string system. The builder assigns positional SSA indices (`Result(stmt, 0)`) and optional creation breaks this scheme. Same class of limitation as `Vec<ResultValue>`.

2. **Remove `-> {result:type}` from format string (rejected):** Would make void-if work by relying on the `%r = ...` prefix for result-carrying ops. But loses explicit type annotation in text format, triggers `T: Placeholder` bound cascade on all SCF generics, and is a breaking change to all existing SCF text format.

3. **Separate `VoidIf`/`VoidFor` enum variants (considered, deferred):** Avoids derive macro changes by using distinct types. Clean but adds API surface and branching in interpret impls.

**Recommended future approach — optional format string syntax:** Add bracket syntax to format strings (e.g., `$if {condition} then {then_body} else {else_body} [-> {result:type}]`) where `[...]` denotes segments conditional on an `Option` field being `Some`. This requires coordinated changes to:
- `kirin-derive-chumsky/src/format.rs` — EBNF parser for `[...]` optional segments
- `kirin-derive-chumsky/src/validation.rs` — allow optional field occurrences
- `kirin-derive-toolkit/src/template/builder_template/helpers.rs` — conditional SSA index allocation for `Option<ResultValue>`
- `kirin-derive-chumsky/src/codegen/emit_ir/field_emit.rs` — conditional emit based on `Option::is_some()`
- `kirin-derive-chumsky/src/codegen/pretty_print/statement.rs` — conditional print

This is a derive infrastructure enhancement (~2-3 days) that would also benefit other dialects needing optional fields in text format. Should be bundled with `Vec<ResultValue>` support (issue #2) since both require builder template changes to the positional SSA indexing scheme.

**Interpreter readiness:** The interpret impl changes are trivial once the derive supports `Option<ResultValue>` — guard `interp.write(self.result, value)` behind `if let Some(result) = self.result`. The `Continuation::Yield(V)` type stays unchanged; void handling is contained in kirin-scf.

---

## 4. `StackInterpreter::eval_block` Only Exits on `Yield` (pre-existing, documented)

**Crate:** kirin-interpreter
**Discovered in:** Wave 5a (while debugging `If` interpreter)

`StackInterpreter::eval_block` calls `run_nested_calls(|_interp, is_yield| is_yield)`, meaning it only returns when it receives `Continuation::Yield`. A `Return` inside an SCF body causes it to try `pop()` on an empty pending_results stack, triggering `InterpreterError::NoFrame`.

**Impact:** SCF block bodies must terminate with `yield`, not `ret`. This is correct per MLIR semantics but is not enforced at the IR level — it's a runtime invariant.

**Deferred (2026-03-22):** Planned to add `eval_block` doc comment enhancement and explore IR-level yield enforcement. Deferred because the doc change was bundled with void-if (#3) which was blocked by derive macro limitations. The doc enhancement should be done standalone: document in `kirin-interpreter/src/block_eval.rs` that SCF body blocks MUST terminate with `Yield`, that `StackInterpreter::eval_block` only returns on `Yield`, and that `Return` inside an SCF body triggers `NoFrame`. A full Verifier trait for IR-level enforcement is a separate architectural initiative.

---

## 5. RAII Scope Guards: `&mut` Borrow Conflicts (solved)

**Crate:** kirin-chumsky
**Discovered in:** Wave 3a

The plan proposed `ScopeGuard<'a, 'b, IR>` holding `&'a mut EmitContext<'b, IR>`. This creates the classic Rust borrow conflict: while the guard exists, nothing else can use `ctx`. The emit functions need to call methods on `ctx` (via the guard) during the guarded section.

**Solution applied:** Implemented `Deref`/`DerefMut` on the guard types so they can be used as `&mut EmitContext`. Callers use `guard.some_method()` instead of `ctx.some_method()` within the guarded section, then explicitly `drop(guard)` before accessing `ctx` directly again.

---

## 6. `Constant` Interpret Impl: `From<T>` → `TryFrom<T>` Cascade

**Crate:** kirin-constant, kirin-arith
**Discovered in:** Wave 5b

Replacing `From<ArithValue> for i64` with `TryFrom` broke `kirin-constant`'s `Constant` interpret impl, which had `I::Value: From<T>` bounds. The fix changed the bound to `I::Value: TryFrom<T, Error: std::error::Error + Send + Sync + 'static>`.

**Why this works:** The standard library provides a blanket `impl<T, U> TryFrom<U> for T where T: From<U>` with `Error = Infallible`. Since `Infallible` implements `std::error::Error`, all existing `From<T>` impls automatically satisfy the new `TryFrom<T>` bound. No downstream breakage.

---

## 7. `Staged` Does Not Implement `Debug` (testing ergonomics)

**Crate:** kirin-interpreter
**Discovered in:** Wave 4 (try_in_stage tests)

`Staged<'a, 'ir, I, L>` doesn't implement `Debug`, which prevents using `unwrap()`, `unwrap_err()`, or `expect()` on `Result<Staged, ...>` in tests. Additionally, `Staged` borrows `&mut` the interpreter, so it must be dropped before the interpreter can be used again.

**Workaround:** Tests use `match` or `is_ok()`/`is_err()` checks with explicit drops instead of the ergonomic unwrap methods.

---

## 8. Worktree Isolation: Agents Landing in Wrong Repos

**Tooling issue, not code**

The `isolation: "worktree"` feature occasionally created worktrees pointing to the wrong repository or branch. The Wave 2 agent landed in a Python project's directory, and Wave 6 initially had the wrong content. Agents had to detect this (via `git rev-parse --show-toplevel` or file existence checks) and recover.

**Mitigation:** All agent prompts include "WORKTREE CHECK" as invariant #0. Agents that detected the wrong repo reported the issue or attempted recovery.

---

## 9. `run_forward` Frame Pop Contract Change

**Crate:** kirin-interpreter
**Discovered in:** Wave 4

The plan called for consuming the frame inside `run_forward` instead of cloning its maps. The complication: `run_forward` previously left the frame on the stack, and the single caller (`call.rs`) popped it afterward. Moving the pop into `run_forward` changes the contract — on error, the frame is still on the stack (for debugging), but on success it's consumed.

**Solution applied:** `run_forward` now pops the frame on success and returns the consumed maps. The caller in `call.rs` was updated to only pop on error paths. The `run_forward` doc comment documents this behavior.

---

## 10. Pre-existing Clippy Warnings (not addressed)

**Crate:** kirin-ir
**3 warnings:** `IdMap` struct and its methods in `gc.rs` are marked dead code after LHF-6 restricted `Arena::gc()` visibility to `pub(crate)`.

These are expected: the GC infrastructure exists but isn't used from outside the crate yet. Left as-is since removing it would be premature if GC is planned for future use.
