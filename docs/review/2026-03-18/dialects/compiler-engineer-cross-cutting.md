# Dialects — Compiler Engineer Cross-Cutting Review

**Crates:** kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-cmp, kirin-function (~3295 lines)

---

## Findings

### D-CC-1. All dialect crates depend on top-level `kirin`, pulling parser+printer unconditionally
**Severity:** P1 | **Confidence:** High
**Files:** `crates/kirin-cf/Cargo.toml:7`, `crates/kirin-arith/Cargo.toml:7` (all 7 dialect crates)

Every dialect crate has `kirin.workspace = true` as a mandatory dependency. The top-level `kirin` crate unconditionally depends on `kirin-chumsky` (with `derive` feature) and `kirin-prettyless` (with `derive` feature). This means a user who only wants the IR types and interpreter for a dialect must still compile the full parser and printer stacks including `chumsky`, `logos`, and `kirin-lexer`. At 50 dialects, this creates a wide compilation fan-out where every dialect's build depends on the entire framework.

**Recommendation:** Dialect crates should depend on `kirin-ir` directly instead of `kirin`. The `kirin` umbrella should be for end-user convenience only. The `HasParser` and `PrettyPrint` derives on dialect types (e.g., `crates/kirin-arith/src/lib.rs:84`) could be feature-gated behind `parser`/`pretty` features, mirroring the existing `interpret` pattern.

### D-CC-2. Dialect crates share identical Cargo.toml structure but no templating
**Severity:** P3 | **Confidence:** Medium
**Files:** All 7 dialect `Cargo.toml` files

Five of seven dialect crates have byte-identical dependency structures (`kirin` + optional `kirin-interpreter`). At 50 dialects this becomes a maintenance burden. Not actionable now but worth noting for future workspace automation.

### D-CC-3. No cross-dependencies between dialect crates
**Severity:** Positive | **Confidence:** High

Dialect crates are fully independent of each other. `kirin-bitwise` does not depend on `kirin-arith`, `kirin-cmp` does not depend on `kirin-cf`, etc. This is correct and supports parallel compilation.

### D-CC-4. Monomorphization at 50 dialects
**Severity:** P3 | **Confidence:** Medium

Each dialect is generic over `T: CompileTimeValue`. When composed into a language enum, the `#[wraps]` delegation generates match arms per variant. With 50 dialects this is O(N) match arms per trait -- acceptable. The `Dialect` derive generates 19 trait impls per dialect type (14 field iters + 5 properties), so at 50 dialects that is ~950 generated trait impls. The template system keeps each impl small, so this should remain manageable.

---

**Summary:** The most impactful finding (D-CC-1) is that dialect crates force the full parser/printer stack through their dependency on the top-level `kirin` crate. Decoupling dialects to depend on `kirin-ir` directly would significantly reduce compile times for interpreter-only and analysis-only use cases.
