# Namespace Keyword Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `{.keyword}` format syntax and namespace threading so dialect composition produces `%res = arith.add %a, %b -> i64` instead of `%res = add %a, %b -> i64`.

**Architecture:** New `FormatElement::Keyword` variant parsed from `{.X}` in format strings. `HasDialectParser` and `PrettyPrint` traits gain `namespaced_parser`/`namespaced_pretty_print` methods that accept `&[&'static str]`. Wrapper codegen threads namespace slices through these methods. Migration updates all ~30 format strings.

**Tech Stack:** Rust proc-macros (syn, quote, darling), chumsky parser combinators, insta snapshot tests.

---

### Task 1: Add `FormatElement::Keyword` to Format Parser

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/format.rs`
- Test: same file (inline `#[cfg(test)]` module)

**Step 1: Write the failing test**

Add to the existing `tests` module in `format.rs`:

```rust
#[test]
fn test_format_parser_keyword() {
    let format = Format::parse("{result:name} = {.add} {lhs}, {rhs} -> {result:type}", proc_macro2::Span::call_site()).unwrap();
    insta::assert_debug_snapshot!(format);
}

#[test]
fn test_format_parser_keyword_only() {
    let format = Format::parse("{.ret} {0}", proc_macro2::Span::call_site()).unwrap();
    insta::assert_debug_snapshot!(format);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p kirin-derive-chumsky -E 'test(test_format_parser_keyword)'`
Expected: FAIL — `{.add}` doesn't parse (the `.` after `{` is unexpected)

**Step 3: Add `FormatElement::Keyword` variant**

In `format.rs`, add to the `FormatElement` enum:

```rust
pub enum FormatElement<'src> {
    Token(Vec<Token<'src>>),
    Field(&'src str, FormatOption),
    Keyword(&'src str),
}
```

**Step 4: Update the format parser to handle `{.X}`**

In `Format::parser()`, add a keyword branch to the interpolation parser (inside the `lbrace` block around line 81). The keyword parser matches `LBrace Dot Identifier RBrace`:

```rust
let keyword = lbrace
    .ignore_then(just(FormatToken::Dot))
    .ignore_then(select! { FormatToken::Identifier(name) => name })
    .then_ignore(rbrace)
    .map(FormatElement::Keyword);
```

Add this as an alternative in the `choice(...)` alongside the existing field interpolation parser.

Note: The format string is lexed by `kirin_lexer`, which already has `Token::Dot`. The format parser uses `FormatToken` which wraps `kirin_lexer::Token`. Check what token type `FormatToken` uses — if it wraps `Token` directly, `Token::Dot` is already available. If it has its own enum, add a `Dot` variant.

**Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p kirin-derive-chumsky -E 'test(test_format_parser_keyword)'`
Expected: FAIL (snapshots don't exist yet)

Run: `cargo insta review` to accept the new snapshots.

Run again: `cargo nextest run -p kirin-derive-chumsky -E 'test(test_format_parser_keyword)'`
Expected: PASS

**Step 6: Verify existing tests still pass**

Run: `cargo nextest run -p kirin-derive-chumsky -E 'test(format)'`
Expected: All existing format parser tests PASS (no regressions)

**Step 7: Commit**

```bash
git add crates/kirin-derive-chumsky/src/format.rs crates/kirin-derive-chumsky/src/snapshots/
git commit -m "feat(derive-chumsky): add FormatElement::Keyword for {.keyword} syntax"
```

---

### Task 2: Update Validation for `FormatElement::Keyword`

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/validation.rs`

**Step 1: Understand the current code**

`validate_format` iterates format elements via `FormatVisitor`. The `visit_format` method (not shown in validation.rs but called) dispatches to `visit_field_occurrence` for `FormatElement::Field` and `visit_tokens` for `FormatElement::Token`. Adding `FormatElement::Keyword` requires the visitor to handle it — likely a no-op since keywords aren't fields.

Read `validation.rs` to find where `FormatElement` variants are matched. The `visit_format` method may be in a trait or the `Format` type itself. Find where the match happens and add a `Keyword(_)` arm that does nothing (keywords are literal tokens, not field references).

**Step 2: Add `Keyword` handling**

Wherever `FormatElement` is matched in the visitor/iteration logic, add:

```rust
FormatElement::Keyword(_) => {
    // Keywords are literal tokens with namespace support, not field references.
    // No validation needed.
}
```

**Step 3: Run all validation tests**

Run: `cargo nextest run -p kirin-derive-chumsky`
Expected: PASS (no regressions)

**Step 4: Commit**

```bash
git add crates/kirin-derive-chumsky/src/validation.rs
git commit -m "feat(derive-chumsky): handle FormatElement::Keyword in validation"
```

---

### Task 3: Add `namespaced_parser` to `HasDialectParser` Trait

**Files:**
- Modify: `crates/kirin-chumsky/src/traits/has_parser.rs`

**Step 1: Add `namespaced_parser` method with default `recursive_parser`**

Current trait (lines 42-66):

```rust
pub trait HasDialectParser<'tokens, 'src: 'tokens>: Sized {
    type Output<TypeOutput, LanguageOutput>: Clone + PartialEq
    where
        TypeOutput: Clone + PartialEq + 'tokens,
        LanguageOutput: Clone + PartialEq + 'tokens;

    fn recursive_parser<I, TypeOutput, LanguageOutput>(
        language: super::RecursiveParser<'tokens, 'src, I, LanguageOutput>,
    ) -> BoxedParser<'tokens, 'src, I, Self::Output<TypeOutput, LanguageOutput>>
    where
        I: TokenInput<'tokens, 'src>,
        TypeOutput: Clone + PartialEq + 'tokens,
        LanguageOutput: Clone + PartialEq + 'tokens;
}
```

Change to:

```rust
pub trait HasDialectParser<'tokens, 'src: 'tokens>: Sized {
    type Output<TypeOutput, LanguageOutput>: Clone + PartialEq
    where
        TypeOutput: Clone + PartialEq + 'tokens,
        LanguageOutput: Clone + PartialEq + 'tokens;

    /// Backward-compatible entry point. Delegates to `namespaced_parser` with empty namespace.
    fn recursive_parser<I, TypeOutput, LanguageOutput>(
        language: super::RecursiveParser<'tokens, 'src, I, LanguageOutput>,
    ) -> BoxedParser<'tokens, 'src, I, Self::Output<TypeOutput, LanguageOutput>>
    where
        I: TokenInput<'tokens, 'src>,
        TypeOutput: Clone + PartialEq + 'tokens,
        LanguageOutput: Clone + PartialEq + 'tokens,
    {
        Self::namespaced_parser::<I, TypeOutput, LanguageOutput>(language, &[])
    }

    /// Parser with namespace prefix applied to `{.keyword}` tokens.
    fn namespaced_parser<I, TypeOutput, LanguageOutput>(
        language: super::RecursiveParser<'tokens, 'src, I, LanguageOutput>,
        namespace: &[&'static str],
    ) -> BoxedParser<'tokens, 'src, I, Self::Output<TypeOutput, LanguageOutput>>
    where
        I: TokenInput<'tokens, 'src>,
        TypeOutput: Clone + PartialEq + 'tokens,
        LanguageOutput: Clone + PartialEq + 'tokens;
}
```

**Step 2: Check for manual `HasDialectParser` impls**

Search for `impl HasDialectParser` or `impl<` ... `HasDialectParser` across the codebase. Any manual impls need to be updated: rename `recursive_parser` to `namespaced_parser` and add the `namespace: &[&'static str]` parameter (ignored for now).

**Step 3: Verify it compiles**

Run: `cargo build --workspace`
Expected: May fail if there are manual impls that still define `recursive_parser`. Fix those first.

**Step 4: Run tests**

Run: `cargo nextest run --workspace`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/kirin-chumsky/src/traits/has_parser.rs
# Also add any files with manual HasDialectParser impls that were updated
git commit -m "feat(chumsky): add namespaced_parser to HasDialectParser trait"
```

---

### Task 4: Add `namespaced_pretty_print` to `PrettyPrint` Trait

**Files:**
- Modify: `crates/kirin-prettyless/src/traits.rs`

**Step 1: Add `namespaced_pretty_print` method**

Current `PrettyPrint` trait (lines 39-69) has `pretty_print`, `pretty_print_name`, `pretty_print_type`.

Add `namespaced_pretty_print` and make `pretty_print` a default method:

```rust
pub trait PrettyPrint {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: core::fmt::Display,
    {
        self.namespaced_pretty_print(doc, &[])
    }

    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: core::fmt::Display;

    // ... existing pretty_print_name, pretty_print_type unchanged ...
}
```

**Step 2: Check for manual `PrettyPrint` impls**

Search for `impl PrettyPrint for` across the codebase. Manual impls currently define `pretty_print` — they need to be renamed to `namespaced_pretty_print` with the extra `namespace` parameter (ignored). Or they can keep `pretty_print` since it has a default now, and add a `namespaced_pretty_print` that ignores namespace. The cleanest approach: change manual impls to implement `namespaced_pretty_print` (ignoring namespace param), so the default `pretty_print` delegates correctly.

Also update the blanket `impl PrettyPrint for &T` (line 205-232) to delegate `namespaced_pretty_print`.

**Step 3: Verify it compiles and tests pass**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/kirin-prettyless/src/traits.rs
# Also add any files with manual PrettyPrint impls that were updated
git commit -m "feat(prettyless): add namespaced_pretty_print to PrettyPrint trait"
```

---

### Task 5: Update Parser Codegen for `FormatElement::Keyword`

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/chain.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/impl_gen.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/generate.rs`

**Step 1: Add keyword parser generation to `chain.rs`**

In `build_parser_chain` (line 39), add a match arm for `FormatElement::Keyword`:

```rust
FormatElement::Keyword(name) => {
    parser_parts.push(ParserPart::Token(self.keyword_parser(name, crate_path)));
}
```

Add the `keyword_parser` method to `GenerateHasDialectParser`:

```rust
fn keyword_parser(&self, name: &str, crate_path: &syn::Path) -> TokenStream {
    quote! {
        {
            let keyword_parser = if namespace.is_empty() {
                #crate_path::chumsky::prelude::just(#crate_path::Token::Identifier(#name)).boxed()
            } else {
                let mut parts = ::std::vec::Vec::new();
                for &ns in namespace.iter() {
                    parts.push(ns);
                }
                parts.push(#name);
                let mut p = #crate_path::chumsky::prelude::just(
                    #crate_path::Token::Identifier(parts[0])
                );
                for &part in &parts[1..] {
                    p = p.then_ignore(#crate_path::chumsky::prelude::just(#crate_path::Token::Dot))
                         .then_ignore(#crate_path::chumsky::prelude::just(
                             #crate_path::Token::Identifier(part)
                         ));
                }
                p.boxed()
            };
            keyword_parser
        }
    }
}
```

**Step 2: Update `impl_gen.rs` to generate `namespaced_parser` instead of `recursive_parser`**

In `generate_dialect_parser_impl` (line 133), change the generated method from `fn recursive_parser` to `fn namespaced_parser` and add `namespace: &[&'static str]` parameter.

The generated impl should look like:

```rust
impl<...> HasDialectParser<'tokens, 'src> for OriginalType {
    type Output<...> = ...;

    fn namespaced_parser<I, TypeOutput, LanguageOutput>(
        language: RecursiveParser<...>,
        namespace: &[&'static str],
    ) -> BoxedParser<...>
    where ...
    {
        // existing body, now with `namespace` in scope
    }
}
```

**Step 3: Thread `namespace` into `build_statement_parser` and `build_parser_chain`**

The `namespace` variable is now in scope in the generated function body. The `keyword_parser` method (Step 1) references it directly in the generated code. No explicit passing needed — `namespace` is a local variable in the generated `namespaced_parser` function.

**Step 4: Update wrapper parser to call `namespaced_parser`**

In `build_wrapper_parser` (generate.rs line 167), change the delegation from:

```rust
WrappedType::recursive_parser::<I, T, L>(language.clone())
```

to (when the wrapper has a namespace format):

```rust
{
    let mut ns = namespace.to_vec();
    ns.push("arith");
    WrappedType::namespaced_parser::<I, T, L>(language.clone(), &ns)
}
```

When the wrapper has no namespace (no format attribute or empty), just pass through:

```rust
WrappedType::namespaced_parser::<I, T, L>(language.clone(), namespace)
```

**Step 5: Verify it compiles**

Run: `cargo build --workspace`
Expected: May initially fail due to snapshot mismatches or compilation errors. Fix iteratively.

**Step 6: Update codegen snapshots**

Run: `cargo nextest run -p kirin-derive-chumsky`
Run: `cargo insta review` — accept updated snapshots showing `namespaced_parser` instead of `recursive_parser`.

**Step 7: Commit**

```bash
git add crates/kirin-derive-chumsky/src/codegen/parser/
git commit -m "feat(derive-chumsky): generate namespaced_parser with keyword-aware chain building"
```

---

### Task 6: Update PrettyPrint Codegen for `FormatElement::Keyword`

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/pretty_print/statement.rs`
- Possibly modify: `crates/kirin-derive-chumsky/src/codegen/pretty_print/` (impl generation file)

**Step 1: Update `generate_format_print` to handle `FormatElement::Keyword`**

In `generate_format_print` (line 260), add a match arm for `Keyword`:

```rust
FormatElement::Keyword(name) => {
    let keyword_expr = quote! {
        {
            let keyword_text = if namespace.is_empty() {
                #name.to_string()
            } else {
                let mut s = namespace.join(".");
                s.push('.');
                s.push_str(#name);
                s
            };
            doc.text(keyword_text)
        }
    };
    // Add spacing logic similar to Token elements
    parts.push(keyword_expr);
}
```

**Step 2: Update impl generation to produce `namespaced_pretty_print`**

The generated `PrettyPrint` impl should implement `namespaced_pretty_print` instead of `pretty_print`:

```rust
impl PrettyPrint for DialectType {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self, doc: &'a Document<'a, L>, namespace: &[&str],
    ) -> ArenaDoc<'a>
    where L::Type: core::fmt::Display
    {
        // existing body with namespace in scope
    }
}
```

**Step 3: Update wrapper branch**

In the enum print match for wrapper variants, thread namespace:

```rust
// Without namespace format:
inner.namespaced_pretty_print(doc, namespace)

// With namespace format "arith":
{
    let mut ns = namespace.to_vec();
    ns.push("arith");
    inner.namespaced_pretty_print(doc, &ns)
}
```

**Step 4: Verify and update snapshots**

Run: `cargo nextest run -p kirin-derive-chumsky && cargo insta review`

**Step 5: Commit**

```bash
git add crates/kirin-derive-chumsky/src/codegen/pretty_print/
git commit -m "feat(derive-chumsky): generate namespaced_pretty_print with keyword-aware printing"
```

---

### Task 7: Migrate All Format Strings to `{.keyword}` Syntax

**Files to modify** (30 format strings across these files):

| Crate | File | Keywords |
|-------|------|----------|
| `kirin-arith` | `src/lib.rs` | add, sub, mul, div, rem, neg |
| `kirin-bitwise` | `src/lib.rs` | and, or, xor, not, shl, shr |
| `kirin-cf` | `src/lib.rs` | br, cond_br (+then, else) |
| `kirin-cmp` | `src/lib.rs` | eq, ne, lt, le, gt, ge |
| `kirin-constant` | `src/lib.rs` | constant |
| `kirin-scf` | `src/lib.rs` | if, then, else, for, in, step, do, yield |
| `kirin-function` | `src/lambda.rs` | lambda, captures |
| `kirin-function` | `src/bind.rs` | bind, captures |
| `kirin-function` | `src/call.rs` | call |
| `kirin-function` | `src/ret.rs` | ret |
| `kirin-function` | `tests/lambda_print.rs` | lambda, captures |
| `example` | `simple.rs` | lambda, captures, if, then, else |

**Migration rule:** Replace bare keyword identifiers with `{.keyword}`. Only the **first identifier after `=`** (or the **first identifier** if no assignment) is the operation keyword. Secondary identifiers like `then`, `else`, `in`, `step`, `do`, `captures` are also keywords if they are literal tokens in the format (not field names).

**IMPORTANT DESIGN DECISION:** Only the **operation keyword** (the one that gets namespace-prefixed) should use `{.keyword}` syntax. Secondary syntax words like `then`, `else`, `in`, `step`, `do`, `captures` that are part of the statement syntax but NOT the operation name should remain as bare tokens. The namespace prefix only applies to the operation keyword.

Review each format string carefully:
- `"{result:name} = add {lhs}, {rhs} -> {result:type}"` → `"{result:name} = {.add} {lhs}, {rhs} -> {result:type}"` (only `add` is the keyword)
- `"if {condition} then {then_body} else {else_body}"` → `"{.if} {condition} then {then_body} else {else_body}"` (only `if` is the keyword)
- `"for {induction_var} in {start}..{end} step {step} do {body}"` → `"{.for} {induction_var} in {start}..{end} step {step} do {body}"` (only `for` is the keyword)
- `"br {target}({args})"` → `"{.br} {target}({args})"` (only `br` is the keyword)
- `"cond_br {condition} then={true_target}(...) else={false_target}(...)"` → `"{.cond_br} {condition} then={true_target}(...) else={false_target}(...)"` (only `cond_br` is the keyword)
- `"{res:name} = lambda {name} captures({captures}) {body} -> {res:type}"` → `"{res:name} = {.lambda} {name} captures({captures}) {body} -> {res:type}"` (only `lambda` is the keyword)
- `"{res:name} = bind {target} captures({captures}) -> {res:type}"` → `"{res:name} = {.bind} {target} captures({captures}) -> {res:type}"` (only `bind` is the keyword)
- `"{res:name} = call {target}({args}) -> {res:type}"` → `"{res:name} = {.call} {target}({args}) -> {res:type}"` (only `call` is the keyword)
- `"ret {value}"` → `"{.ret} {value}"` (only `ret` is the keyword)
- `"yield {value}"` → `"{.yield} {value}"` (only `yield` is the keyword)

**Step 1: Apply all migrations**

Edit each file, replacing the format strings per the rules above.

**Step 2: Verify everything compiles and tests pass**

Run: `cargo build --workspace && cargo nextest run --workspace`
Run: `cargo insta review` if any snapshots changed.
Expected: PASS (format strings parse correctly, codegen produces correct output)

**Step 3: Commit**

```bash
git add -A
git commit -m "refactor: migrate all format strings to {.keyword} syntax"
```

---

### Task 8: Integration Tests

**Files:**
- Create: `crates/kirin-chumsky/tests/namespace_roundtrip.rs` (or add to existing test file)
- Modify: `crates/kirin-test-languages/src/` (add test language with namespace)

**Step 1: Create a test dialect and wrapper with namespace**

In `kirin-test-languages`, create a simple test language that uses `#[wraps]` with `#[chumsky(format = "arith")]` to wrap an arithmetic dialect. The inner dialect should use `{.add}` in its format string.

**Step 2: Write roundtrip test**

Test that parsing `%res = arith.add %a, %b -> i64` and printing it back produces the same string.

**Step 3: Write multi-level nesting test**

Create a two-layer wrapper: `TopLevel` wraps `MyLang` (format = "lang"), `MyLang` wraps `Arith` (format = "arith"). Test that `%res = lang.arith.add %a, %b -> i64` roundtrips correctly.

**Step 4: Write standalone test (no namespace)**

Test that a dialect with `{.add}` but no wrapper produces `%res = add %a, %b -> i64` (no prefix).

**Step 5: Run and review**

Run: `cargo nextest run --workspace`
Run: `cargo insta review` if snapshot tests are used.
Expected: All PASS

**Step 6: Commit**

```bash
git add crates/kirin-test-languages/ crates/kirin-chumsky/tests/
git commit -m "test: add namespace roundtrip and multi-level nesting integration tests"
```

---

### Task 9: Final Verification

**Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: Clean build, no warnings

**Step 2: Full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass

**Step 3: Doc tests**

Run: `cargo test --doc --workspace`
Expected: All pass

**Step 4: Format check**

Run: `cargo fmt --all`

**Step 5: Commit any formatting changes**

```bash
git add -A
git commit -m "chore: format code"
```
