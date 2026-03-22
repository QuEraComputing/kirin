# Low-Hanging Fruit Plan Template

Use this template when generating the `low-hanging-fruit.md` plan file for
quick fixes that need no design work. All items are executed sequentially by
a single agent before wave work begins.

---

## Template

```markdown
# Low-Hanging Fruit

**Review report:** `<path to review report>`
**Execution:** Single agent, sequential, review changes after completion.
**Estimated total effort:** <sum of individual estimates>

---

## Items

### LHF-1: <Title> (<finding ID>)

**Issue:** <1-2 sentences describing the problem>
**Crate:** <crate> | **File:** `<file:line>`

**Change:**
<Specific change to make. Be precise — name the function, type, or pattern
to modify. Include the target state, not just "fix it".>

**Validation:**
```bash
<exact command to verify this item>
```

**Must not do:** <key constraint, if any — omit if none>

---

### LHF-2: <Title> (<finding ID>)
...

---

## Execution Order

Execute items in the order listed. Each item is independent, but the order
minimizes churn (e.g., renames before downstream changes).

If any item fails validation, stop and report before continuing — do not
skip items.

## Clippy and Warnings Policy

Do NOT introduce `#[allow(...)]` annotations to suppress warnings — fix the
underlying cause. Do NOT use workarounds (renaming to `_var`, dead code
annotations). If a suppression seems genuinely necessary, stop and report
to the lead with the root cause explanation.

## Final Validation

After all items:
```bash
cargo clippy --workspace                  # must be warning-free
cargo nextest run --workspace
cargo test --doc --workspace
```

No snapshot test changes unless explicitly expected (if so, run
`cargo insta test` and report changes).

## Success Criteria

All items pass their individual validation commands. Final workspace clippy
and tests pass. Each change is minimal and self-evident — no design decisions
were required.
```

---

## Classification criteria

An item qualifies as low-hanging fruit when ALL of these hold:
- Estimated effort < 30 minutes
- Single file or small set of closely related files
- No design decisions — the change is mechanical or the review report
  specifies the exact action
- No cross-crate impact beyond import updates
- The review report classifies it as "Quick Win" or equivalent

Common low-hanging fruit patterns:
- Renames (`ChumskyError` -> `TextParseError`)
- Adding `#[must_use]` or `#[doc(hidden)]`
- Promoting `debug_assert!` to `assert!`
- Adding doc comments
- Adding duplicate-name checks with clear error types
