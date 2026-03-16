---
name: kirin-derive-macros
description: Use when building or debugging kirin derive macros. Reuses kirin-derive-toolkit's code-block algebra, IR model, and generators. Escalates to /refactor if the toolkit's design makes the new derive too complicated.
---

# Kirin Derive Macros

## Overview

Skill for developing new derive macros in the kirin ecosystem. The primary directive is to **reuse `kirin-derive-toolkit` infrastructure as much as possible** — its IR model, code-block algebra, Scan/Emit traits, and utility functions. If the existing toolkit design makes a new derive unreasonably complex, the skill escalates rather than shoe-horning.

**Announce at start:** "I'm using the kirin-derive-macros skill to guide this derive macro work."

## When to Use

- Explicit: user invokes `/kirin-derive-macros`
- Auto-suggest when detecting:
  - New `#[derive(...)]` macro for a kirin crate
  - Changes to existing kirin derive proc-macro crates
  - Debugging expanded kirin derive output

**Don't use for:**
- Using existing derive macros (just apply them)
- Non-kirin proc-macro work (use general Rust proc-macro knowledge instead)
- Refactoring the toolkit itself (use `/refactor`)

## Core Directive: Reuse the Toolkit

Before writing any code, explore what `kirin-derive-toolkit` already provides. Read these in order:

1. `crates/kirin-derive-toolkit/src/lib.rs` — public API surface
2. `crates/kirin-derive-toolkit/src/tokens/` — code-block types (`TraitImpl`, `MatchExpr`, `Method`, etc.)
3. `crates/kirin-derive-toolkit/src/ir/` — IR model (`Input<L>`, `Statement<L>`, `FieldInfo<L>`, `Layout`)
4. `crates/kirin-derive-toolkit/src/emit.rs` — `Emit` trait (visitor pattern for code generation)
5. `crates/kirin-derive-toolkit/src/scan.rs` — `Scan` trait (visitor pattern for info collection)
6. `crates/kirin-derive-toolkit/src/codegen/` — utilities (`combine_where_clauses`, generics helpers)

**Checklist before writing new helpers:**
- [ ] Is there an existing code-block type that fits? (`TraitImpl`, `MatchExpr`, `Method`, `MatchArm`)
- [ ] Is there an existing utility for this? (`combine_where_clauses`, crate path resolution, pattern building)
- [ ] Can `Scan`/`Emit` with `StandardLayout` handle this, or do I need a custom `Layout`?
- [ ] Does an existing derive crate solve a similar problem I can reference?

### Reference Implementations

| Crate | Demonstrates |
|-------|-------------|
| `kirin-derive-ir` | `StandardLayout`, Generator chain, property/field-iter/builder/marker generators |
| `kirin-derive-interpreter` | Custom `Layout` (`EvalCallLayout`), `Scan`/`Emit` pattern, wrapper delegation with extra type params |
| `kirin-derive-chumsky` | Complex code generation, format-driven visitor, absorbing a library into a derive crate |
| `kirin-derive-prettyless` | Simplest possible derive — thin proc-macro entry point |

## Complexity Gate: When to Stop and Escalate

**If any of these are true, STOP implementing and escalate:**

1. You need a code-block type that doesn't exist in `tokens/` and can't be expressed by composing existing ones
2. The `Scan`/`Emit` trait methods don't fit the traversal pattern you need (e.g., you need multi-pass or cross-statement coordination that `Scan` can't handle)
3. You're fighting `TraitImpl` or `MatchExpr` to produce the output shape you need (e.g., needing conditional items, multiple impl blocks per variant, or non-standard impl structures)
4. The `Layout` trait's extension points (`StatementExtra`, `FieldExtra`, `InputExtra`) aren't sufficient for your attribute needs
5. You're duplicating more than 20 lines of logic that already exists in another derive crate but isn't factored into the toolkit

### Escalation Procedure

When the toolkit design makes the new derive too complicated:

1. **Document the problem** concretely — show what you're trying to generate, what toolkit API you tried, and why it doesn't fit. Include code examples of both what you want and what the toolkit forces you to write.

2. **Save your work** in a worktree:
   ```bash
   # If not already in a worktree, create one
   git stash  # or commit WIP
   ```
   Preserve all progress so it can be resumed after the toolkit is improved.

3. **Report to the user** with:
   ```
   ## Toolkit Limitation Report

   **Derive being implemented:** [name]
   **What I need to generate:**
   [example of desired output code]

   **What the toolkit provides:**
   [API I tried to use]

   **Why it doesn't fit:**
   [concrete explanation with code showing the mismatch]

   **Suggested toolkit change:**
   [specific proposal — new code-block type, Emit method, Layout extension, etc.]

   **Work saved at:** [worktree path or branch name]
   ```

4. **Suggest using `/refactor`** to improve `kirin-derive-toolkit` before continuing the derive implementation. The refactor should target the specific gap identified.

5. **Do NOT work around the limitation** by:
   - Writing raw `quote!` composition for structural code
   - Duplicating toolkit internals in the derive crate
   - Adding one-off helpers that should be in the toolkit
   - Using `proc_macro2::TokenStream` as an escape hatch for typed code blocks

## Architecture: Two-Crate Pattern

Every kirin derive lives in a proc-macro crate that depends on `kirin-derive-toolkit`:

```
kirin-derive-toolkit    (regular lib — IR, code blocks, utilities)
    ^
kirin-derive-<name>     (proc-macro = true — entry points only)
```

### Proc-Macro Crate Structure

Minimal entry point that delegates to toolkit:

```rust
#[proc_macro_derive(MyTrait, attributes(kirin, wraps))]
pub fn derive_my_trait(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    manyhow::function!(|ast: syn::DeriveInput| -> darling::Result<proc_macro2::TokenStream> {
        let ir = Input::<StandardLayout>::from_derive_input(&ast)?;
        // Use toolkit generators or Scan/Emit
        // ...
    })(&ast)
    .into()
}
```

### When to Use Custom Layout

Only define a custom `Layout` when your derive needs per-statement or per-field attributes beyond what `StandardLayout` provides. Examples:

- `EvalCallLayout` adds `#[callable]` flag per variant and `callable_all` at input level
- `ChumskyLayout` adds format strings and parser-specific field metadata

If `StandardLayout` (all `()` extras) works, use it.

## Code Generation with Toolkit Types

### Use Typed Code Blocks

Always use toolkit's typed structs over raw `quote!` for structural code:

```rust
// Use TraitImpl, Method, MatchExpr, MatchArm from kirin_derive_toolkit::tokens
let trait_impl = TraitImpl::new(generics, trait_path, type_name)
    .type_generics(ty_generics)
    .where_clause(combined_where)
    .method(Method {
        name: syn::parse_quote! { interpret },
        self_arg: quote! { &self },
        params: vec![quote! { interpreter: &mut I }],
        return_type: Some(quote! { Result<V, E> }),
        body: quote! { #match_expr },
    });
```

Raw `quote!` is fine for **leaf expressions** (method bodies, individual expressions), but not for structural code (impl blocks, match expressions, method signatures).

### Critical: type_generics vs impl_generics

When impl generics differ from the type's own generics (e.g., adding `'__ir`, `__InterpI`), you MUST override type_generics:

```rust
let impl_generics = add_extra_params(&base_generics);
let (_, ty_generics_raw, orig_where) = base_generics.split_for_impl();
let ty_generics = ty_generics_raw.to_token_stream(); // early conversion — TypeGenerics lacks Copy

let trait_impl = TraitImpl::new(impl_generics, trait_path, type_name)
    .type_generics(ty_generics.clone()); // MUST override — otherwise extra params leak into type position
```

### Shared Helper Attributes

Multiple derive macros share the same helper attributes — treat these as builtins:

- **`#[kirin(...)]`** — the carry attribute for dialect-specific options (crate path, result fields, terminator flag, etc.). Parsed by darling. Every kirin derive should declare this in its `attributes(...)` list.
- **`#[wraps]`** — bare flag indicating wrapper/delegation pattern. Shared across `Dialect`, `Interpretable`, `CallSemantics`, `HasParser`, `PrettyPrint`. Parsed manually (`attrs.iter().any(|a| a.path().is_ident("wraps"))`).
- **`#[callable]`** — bare flag for interpreter call semantics. Used by `CallSemantics` derive.

These are intentionally separate: `#[kirin(...)]` uses darling's structured parsing, while `#[wraps]` and `#[callable]` are bare flags that compose independently across derives. A type can use `#[wraps]` with both `#[derive(Dialect)]` and `#[derive(Interpretable)]` without coupling those derives.

**When adding a new derive:** reuse existing helper attributes where they apply. Only introduce new attributes when the semantics genuinely differ from what `#[kirin(...)]`, `#[wraps]`, and `#[callable]` already express. If you need a new bare flag, follow the same manual parsing pattern.

### Darling Re-export Rule

Always import darling through the toolkit:
```rust
use kirin_derive_toolkit::prelude::darling;
```
Never add `darling` as a direct dependency — the workspace has multiple versions.

## Scan/Emit Pattern

Use when you need to collect info across all statements before generating code:

```rust
struct MyDerive {
    statements: IndexMap<String, MyStatementInfo>,
}

impl Scan<MyLayout> for MyDerive {
    fn scan_statement(&mut self, stmt: &Statement<MyLayout>) -> darling::Result<()> {
        // Collect per-statement info
    }
}

impl<'ir> Emit<'ir, MyLayout> for MyDerive {
    fn emit_struct(&mut self, data: &'ir DataStruct<MyLayout>) -> darling::Result<TokenStream> {
        // Generate code using collected info + toolkit code blocks
    }
    fn emit_enum(&mut self, data: &'ir DataEnum<MyLayout>) -> darling::Result<TokenStream> {
        // ...
    }
}
```

## Testing

### Snapshot Tests (Primary)

```rust
#[test]
fn test_my_derive() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(crate = kirin_ir)]
        struct MyOp { /* ... */ }
    };
    let ir = Input::<StandardLayout>::from_derive_input(&input).unwrap();
    let output = generate(&ir).unwrap();
    let formatted = rustfmt_token_stream(&output);
    insta::assert_snapshot!(formatted);
}
```

### Debug Dump

`KIRIN_EXPAND_DEBUG=1 cargo nextest run -p kirin-derive-<name>`

### Compile Tests

Add a real type using the derive in `kirin-test-languages` to verify trait bounds and lifetimes resolve correctly.

## Common Pitfalls

1. **Forgetting `attributes(...)` in `proc_macro_derive`** — helper attributes must be declared
2. **Multiple darling versions** — always use toolkit re-export
3. **`TypeGenerics` lacks Copy** — convert to `TokenStream` early via `.to_token_stream()`
4. **Where clause combination** — use `combine_where_clauses(Some(&extra), orig_where)`
5. **Crate path resolution** — default is `::kirin::ir`, tests use `#[kirin(crate = kirin_ir)]`
6. **Wrapper delegation** — `#[wraps]` destructures self, delegates to inner type, adds inner type's trait bound to where clause

## Checklist: New Derive Macro

1. [ ] Read toolkit's public API — identify reusable types and utilities
2. [ ] Decide: `StandardLayout` or custom `Layout`?
3. [ ] Decide: `Scan`/`Emit` or direct generation?
4. [ ] Create proc-macro crate with correct `attributes(...)` list
5. [ ] Implement using toolkit code blocks (`TraitImpl`, `MatchExpr`, etc.)
6. [ ] Handle both struct and enum cases
7. [ ] Handle `#[wraps]` delegation if applicable
8. [ ] Override `.type_generics()` if impl generics differ from type generics
9. [ ] Add snapshot tests with `insta`
10. [ ] Add compile test in `kirin-test-languages`
11. [ ] Verify expanded code with `KIRIN_EXPAND_DEBUG=1`
12. [ ] **Complexity check**: did you stay within toolkit's design, or did you work around it? If the latter, escalate.

## Integration

**Escalates to:**
- `/refactor` — when toolkit design needs improvement to support the new derive

**Pairs with:**
- `/test-driven-development` — snapshot-first workflow
- `/triage-review` — multi-perspective review of derive changes

**Key paths:**
- `crates/kirin-derive-toolkit/` — toolkit (IR, code blocks, utilities)
- `crates/kirin-derive-ir/` — reference: StandardLayout + Generator chain
- `crates/kirin-derive-interpreter/` — reference: custom Layout + Scan/Emit
- `crates/kirin-derive-chumsky/` — reference: complex derive with format visitor
- `crates/kirin-derive-prettyless/` — reference: minimal derive
