# Option D: Minimal Fix — Single Lifetime Only

## Approach

Apply only Change 1 from Option A (collapse `HasParser<'tokens, 'src>` to `HasParser<'t>`)
plus Change 2 (`ParseStatementText<'t, L>`), but skip the monomorphic dispatch (Change 3)
and custom parser hooks (Change 4).

This is the minimum change that might fix the HRTB issue for statement-level parsing while
keeping the existing pipeline dispatch architecture.

### What Changes

1. **`HasParser<'t>`** — single lifetime everywhere
2. **`ParseStatementText<'t, L>`** — lifetime parameter eliminates HRTB for statements
3. **`HasDialectParser<'t>`** — follows from single lifetime
4. **`HasDialectEmitIR<'t, L>`** — already uses single lifetime, now consistent

### What Stays the Same

- `SupportsStageDispatchMut` — still needed for pipeline parsing
- `Pipeline<S>` impl — still uses generic dispatch with HRTB
- `FirstPassAction` / `SecondPassSpecializeAction` — still carry HRTB bounds

### Does This Fix the Problem?

**For statement parsing:** Yes. With `ParseStatementText<'t, L>`, the caller provides concrete
`'t`, so no HRTB is needed. Block/Region types can use `#[wraps]` when parsing single
statements.

**For pipeline parsing:** **Probably not.** The pipeline still needs to dispatch across stages
generically, which requires:
```rust
for<'a, 'src> S: SupportsStageDispatchMut<FirstPassAction<'a, 'src>, ...>
```

Even with single lifetime, this becomes:
```rust
for<'a, 't> S: SupportsStageDispatchMut<FirstPassAction<'a, 't>, ...>
```

The `for<'t>` HRTB on `L: HasParser<'t>` still triggers the overflow.

**Verdict:** This is a partial fix. It helps statement-level parsing but does NOT fix pipeline
parsing. Languages used only with `parse_statement` (not `parse_pipeline`) would benefit.

### Workaround for Pipeline

If pipeline parsing still has HRTB, users who need pipeline parsing with Block/Region types
must continue inlining those variants. This is the status quo for toy-lang's `main.rs` which
uses pipeline parsing.

## Pros

- **Smallest change** — only trait signatures change, no new derives
- **Low risk** — mechanical signature updates
- **Simplifies codegen** — single lifetime removes dual-bound maintenance
- **Quick to implement** — a few days
- **Good stepping stone** — can later add monomorphic dispatch (Change 3) incrementally

## Cons

- **Partial fix only** — pipeline parsing still has HRTB, still can't use `#[wraps]` with pipeline
- **Doesn't fix the toy-lang use case** — toy-lang uses pipeline parsing
- **Existing complexity preserved** — `SupportsStageDispatchMut` and the two-pass pipeline remain
- **No custom parser hooks** — still need fully manual impls for complex parsers
- **Migration effort without full benefit** — updating all `HasParser` impls for only partial fix
