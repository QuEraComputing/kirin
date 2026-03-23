# Builder Template — Lift Vec/Option/SmallVec ResultValue Rejection

**Finding(s):** W1
**Wave:** 0
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

The builder template in `kirin-derive-toolkit` currently rejects `Vec<ResultValue>` and `Option<ResultValue>` fields with explicit compile errors. This blocks multi-result operations like `Call { results: Vec<ResultValue> }` and void-capable operations like `If { result: Option<ResultValue> }`.

The rejection must be lifted and replaced with dynamic SSA allocation codegen for `Vec` and conditional allocation for `Option`.

**Crate(s):** kirin-derive-toolkit
**File(s):**
- `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs` (lines 279-296 for `let_name_eq_result_value`, lines 423-441 for `build_result_impl`)
- `crates/kirin-derive-toolkit/src/ir/fields/collection.rs` (Collection enum — may need SmallVec variant)

**Confidence:** confirmed

## Guiding Principles

- "Derive Infrastructure Conventions": `mod.rs` should stay lean — only module declarations, re-exports, and prelude definitions. Move substantial logic into sibling files.
- "No unsafe code": All implementations MUST use safe Rust.
- "Auto-placeholder for ResultValue fields": ResultValue fields without explicit `#[kirin(type = ...)]` auto-default to `ir_type::placeholder()`. The derive adds `T: Placeholder` to generated builder and EmitIR where clauses automatically.

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs` | modify | Remove Vec/Option rejection in `let_name_eq_result_value` and `build_result_impl`, add dynamic allocation codegen |
| `crates/kirin-derive-toolkit/src/ir/fields/collection.rs` | possibly modify | May need SmallVec variant if design requires it |

**Files explicitly out of scope:**
- `crates/kirin-derive-chumsky/` — format DSL changes are in a separate plan (wave-0/format-dsl-plan.md)
- `crates/kirin-scf/`, `crates/kirin-function/` — dialect changes are in wave-2 plans

## Verify Before Implementing

- [ ] **Verify: `let_name_eq_result_value` rejection location**
  Run: `grep -n "ResultValue field cannot be" crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs`
  Expected: Two locations — one in `let_name_eq_result_value` (~line 283) and one in `build_result_impl` (~line 427)
  If this fails, STOP and report — the rejection may have moved.

- [ ] **Verify: Collection enum has no SmallVec variant yet**
  Run: `grep -n "SmallVec" crates/kirin-derive-toolkit/src/ir/fields/collection.rs`
  Expected: No matches
  If matches found, adapt approach to use existing variant.

- [ ] **Verify: FieldCategory::Result detection works for Vec and Option**
  Run: `grep -n "FieldCategory::Result" crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs`
  Expected: Multiple hits — the code already filters for Result fields, the rejection is inside a match on `collection`.

## Regression Test

- [ ] **Write regression test for Vec<ResultValue> rejection**
  Create a test in `kirin-derive-toolkit` that attempts to use `Vec<ResultValue>` in a struct with `#[derive(Dialect)]` and `#[kirin(builders)]`. Currently this produces a compile error. After the fix, it should compile and produce correct builder codegen.
  Test approach: Add a codegen snapshot test that verifies the generated builder for a struct with `results: Vec<ResultValue>`. Before the fix, this will show compile_error tokens in the snapshot.
  Test file: inline `#[cfg(test)]` in `helpers.rs` or a new test in the builder_template module.

- [ ] **Run the test — confirm it demonstrates the issue**
  Run: `cargo nextest run -p kirin-derive-toolkit -E 'test(vec_result)'`
  Expected: Test captures the compile_error! output in snapshot.

## Design Decisions

**Decision 1: Vec<ResultValue> dynamic count source**
- **Primary approach:** The generated builder function accepts a `result_count: usize` parameter for `Vec<ResultValue>` fields. This is the simplest — the caller specifies how many results they want.
- **Fallback:** Infer the count from a related field (e.g., `init_args.len()` for `For`). This would require a `#[kirin(result_count = field)]` attribute.
- **How to decide:** Start with explicit `result_count` parameter. If downstream dialect usage (W7/W8) shows this is too verbose, add the inference attribute later.

**Decision 2: SmallVec<[ResultValue; N]> support**
- **Primary approach:** Defer SmallVec support. The design doc lists it but `Vec<ResultValue>` covers all use cases. SmallVec optimization can be added later without API breakage.
- **Fallback:** Add a `Collection::SmallVec(usize)` variant now.
- **How to decide:** Check if any dialect in the design doc actually uses `SmallVec<[ResultValue; N]>` directly (answer: no — they all use `Vec<ResultValue>`). Defer.

**Decision 3: Build result struct for Vec<ResultValue> fields**
- **Primary approach:** The build result struct has `pub results: Vec<ResultValue>` (mirroring the dialect struct). The `From<BuildResult> for Statement` impl remains unchanged (it just extracts `id`).
- **Fallback:** N/A — this is the natural representation.
- **How to decide:** Implement directly.

## Implementation Steps

- [ ] **Step 1: Write snapshot test for Vec<ResultValue> builder codegen**
  Add a test struct with `results: Vec<ResultValue>` and capture the generated builder code as a snapshot. This will initially show `compile_error!` tokens.

- [ ] **Step 2: Run test to confirm it captures the rejection**
  Run: `cargo nextest run -p kirin-derive-toolkit -E 'test(vec_result)'`
  Expected: Snapshot captured with compile_error in output.

- [ ] **Step 3: Modify `let_name_eq_result_value` to handle Vec**
  In `helpers.rs`, replace the `Collection::Vec` branch (currently emitting `compile_error!`) with code that generates dynamic SSA allocation:
  ```rust
  Collection::Vec => {
      let name = result_name_map.get(&field.index)...;
      let count_param = format_ident!("{}_count", name);
      results.push(quote! {
          let #name: Vec<ResultValue> = (0..#count_param).map(|i| {
              stage.ssa()
                  .kind(#crate_path::BuilderSSAKind::Result(#statement_id, #base_index + i))
                  .ty(Lang::Type::from(#ssa_ty))
                  .new()
                  .into()
          }).collect();
      });
      // Track that result_index advances by count_param (dynamic)
  }
  ```
  Note: The exact codegen will depend on how `result_index` is tracked. For Vec fields, `result_index` must be tracked dynamically at runtime.

- [ ] **Step 4: Modify `let_name_eq_result_value` to handle Option**
  Replace the `Collection::Option` branch with conditional allocation:
  ```rust
  Collection::Option => {
      let name = result_name_map.get(&field.index)...;
      let has_param = format_ident!("has_{}", name);
      results.push(quote! {
          let #name: Option<ResultValue> = if #has_param {
              Some(stage.ssa()
                  .kind(#crate_path::BuilderSSAKind::Result(#statement_id, #index))
                  .ty(Lang::Type::from(#ssa_ty))
                  .new()
                  .into())
          } else {
              None
          };
      });
  }
  ```

- [ ] **Step 5: Modify `build_result_impl` to handle Vec and Option**
  Update the `build_result_impl` function to emit `pub results: Vec<ResultValue>` and `pub result: Option<ResultValue>` fields instead of `compile_error!`.

- [ ] **Step 6: Modify `build_fn_inputs` to add count/flag parameters**
  For `Vec<ResultValue>` fields, add a `result_count: usize` parameter to the generated builder function. For `Option<ResultValue>`, add a `has_result: bool` parameter (or use a more ergonomic approach like inferring from optional format section presence).

- [ ] **Step 7: Update `result_names` to handle Vec/Option result names**
  The `result_names` function returns identifiers for result fields. Vec fields should return a single name (the Vec field name). Option fields should return a single name.

- [ ] **Step 8: Run all tests**
  Run: `cargo nextest run -p kirin-derive-toolkit`
  Expected: All tests pass, snapshot updated to show correct Vec/Option codegen.

- [ ] **Step 9: Run workspace build to check downstream**
  Run: `cargo build --workspace`
  Expected: Clean build (no downstream crates use Vec/Option ResultValue yet).

- [ ] **Step 10: Fix any clippy warnings**
  Run: `cargo clippy -p kirin-derive-toolkit`
  Expected: No warnings.

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations to suppress warnings — fix the underlying cause.
- Do NOT leave clippy warnings. Run `cargo clippy -p kirin-derive-toolkit` before reporting completion.
- Do NOT add SmallVec support in this plan — defer to a follow-up if needed. Vec covers all design-doc use cases.
- Do NOT modify the `Collection` enum unless strictly necessary — Vec and Option are already variants.
- Do NOT change any dialect struct definitions (kirin-scf, kirin-function) — those are in wave-2 plans.
- No unsafe code (AGENTS.md: all implementations MUST use safe Rust).

## Validation

**Per-step checks:**
- After step 2: `cargo nextest run -p kirin-derive-toolkit -E 'test(vec_result)'` — Expected: snapshot showing compile_error
- After step 8: `cargo nextest run -p kirin-derive-toolkit` — Expected: all tests pass
- After step 9: `cargo build --workspace` — Expected: clean build

**Final checks:**
```bash
cargo clippy -p kirin-derive-toolkit          # Expected: no warnings
cargo nextest run -p kirin-derive-toolkit     # Expected: all tests pass
cargo build --workspace                        # Expected: clean build
cargo test --doc -p kirin-derive-toolkit      # Expected: all doctests pass
```

**Snapshot tests:** yes — run `cargo insta test -p kirin-derive-toolkit` and report changes, do NOT auto-accept.

## Success Criteria

1. `Vec<ResultValue>` fields in dialect structs with `#[kirin(builders)]` generate valid builder functions that dynamically allocate N SSA result values.
2. `Option<ResultValue>` fields generate builder functions with conditional SSA allocation.
3. Existing single `ResultValue` field behavior is unchanged (no regression).
4. Generated code compiles and produces correct `BuildResult` structs.
5. No clippy warnings, no `#[allow(...)]` annotations added.

**Is this a workaround or a real fix?**
This is the real fix. The current `compile_error!` rejection was an intentional limitation that this change removes by implementing proper codegen for collection-wrapped ResultValue fields.
