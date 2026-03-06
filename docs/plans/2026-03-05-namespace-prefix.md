# Namespace Prefix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `#[chumsky(format = "arith")]` support on `#[wraps]` variants to prefix all inner statement keywords with `arith.` (e.g., `arith.add` instead of `add`).

**Architecture:** Three changes: (1) Add `Token::Dot` to the lexer and remove `.` and `$` from identifier character classes, (2) Modify parser codegen to prepend `Identifier("prefix") Dot` before delegated wrapper parsers when format is present, (3) Modify pretty-print codegen to prepend `"prefix."` text before delegated wrapper prints. Validation ensures format on wraps variants is a single identifier.

**Tech Stack:** kirin-lexer (logos), kirin-derive-chumsky (proc-macro codegen with quote/syn), kirin-derive-toolkit (IR model)

---

### Task 1: Add Token::Dot to Lexer

**Files:**
- Modify: `crates/kirin-lexer/src/lib.rs`

**Step 1: Write a failing test for dot tokenization**

Add to the `tests` module at the bottom of `crates/kirin-lexer/src/lib.rs`:

```rust
#[test]
fn test_dot_token() {
    let input = "arith.add";
    let mut lexer = Token::lexer(input);
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("arith"))));
    assert_eq!(lexer.next(), Some(Ok(Token::Dot)));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("add"))));
    assert_eq!(lexer.next(), None);
}

#[test]
fn test_dot_dot_still_works() {
    let input = "a..b";
    let mut lexer = Token::lexer(input);
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("a"))));
    assert_eq!(lexer.next(), Some(Ok(Token::DotDot)));
    assert_eq!(lexer.next(), Some(Ok(Token::Identifier("b"))));
    assert_eq!(lexer.next(), None);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p kirin-lexer -E 'test(test_dot)'`
Expected: FAIL — `Token::Dot` doesn't exist yet.

**Step 3: Add Token::Dot and clean up identifier regex**

In `crates/kirin-lexer/src/lib.rs`:

1. Remove `.` and `$` from all identifier-like regexes:
   - `SSAValue`: change `r"%[\p{XID_Continue}_$.]+"`  to `r"%[\p{XID_Continue}_]+"`
   - `Block`: change `r"\^[\p{XID_Continue}_$.]+"` to `r"\^[\p{XID_Continue}_]+"`
   - `Identifier`: change `r"[\p{XID_Start}_][\p{XID_Continue}_$.]*"` to `r"[\p{XID_Start}_][\p{XID_Continue}_]*"`
   - `Symbol`: change `r"@[\p{XID_Continue}_$.]+"` to `r"@[\p{XID_Continue}_]+"`
   - `AttrId`: change `r"#[\p{XID_Continue}_$.]+"` to `r"#[\p{XID_Continue}_]+"`

2. Add `Token::Dot` variant (before `DotDot` so logos priority is correct — longest match wins for `..`):
   ```rust
   #[token(".")]
   Dot,
   ```

3. Add `Display` arm:
   ```rust
   Token::Dot => write!(f, "."),
   ```

4. Add `ToTokens` arm (under `#[cfg(feature = "quote")]`):
   ```rust
   Token::Dot => {
       tokens.extend(quote::quote! { Token::Dot });
   }
   ```

**Step 4: Run lexer tests**

Run: `cargo nextest run -p kirin-lexer`
Expected: All pass. The `test_dot_token` and `test_dot_dot_still_works` tests pass. Existing tests still pass since none use `.` or `$` in identifiers.

**Step 5: Run full workspace tests to check for breakage**

Run: `cargo nextest run --workspace`
Expected: All pass. If any snapshot tests change, review with `cargo insta review`.

**Step 6: Commit**

```bash
git add crates/kirin-lexer/src/lib.rs
git commit -m "feat(lexer): add Token::Dot and remove . $ from identifier classes"
```

---

### Task 2: Add Namespace Validation for Wraps Variants

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/helpers.rs`

**Step 1: Write a helper function to extract and validate namespace**

Add to `crates/kirin-derive-chumsky/src/codegen/helpers.rs`:

```rust
/// Extracts a namespace prefix from a `#[chumsky(format = "...")]` on a `#[wraps]` variant.
///
/// Returns `Ok(Some(namespace))` if format is present and valid (single identifier),
/// `Ok(None)` if no format attribute, or `Err` if the format string is invalid for a wraps variant.
pub(crate) fn namespace_for_wrapper<L>(
    ir_input: &kirin_derive_toolkit::ir::Input<L>,
    stmt: &kirin_derive_toolkit::ir::Statement<L>,
) -> syn::Result<Option<String>>
where
    L: Layout<ExtraStatementAttrs = ChumskyStatementAttrs>,
    L::ExtraGlobalAttrs: HasGlobalFormat,
{
    let Some(format_str) = format_for_statement(ir_input, stmt) else {
        return Ok(None);
    };

    // Validate: must be a single identifier (no dots, no braces, no spaces)
    let trimmed = format_str.trim();
    if trimmed.is_empty() {
        return Err(syn::Error::new(
            stmt.name.span(),
            "format on a #[wraps] variant must be a single identifier (namespace prefix), got empty string",
        ));
    }

    // Check it's a valid identifier: starts with XID_Start or _, continues with XID_Continue or _
    let mut chars = trimmed.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return Err(syn::Error::new(
            stmt.name.span(),
            format!(
                "format on a #[wraps] variant must be a single identifier (namespace prefix), \
                 got \"{}\"",
                trimmed
            ),
        ));
    }

    for ch in chars {
        if !ch.is_alphanumeric() && ch != '_' {
            return Err(syn::Error::new(
                stmt.name.span(),
                format!(
                    "format on a #[wraps] variant must be a single identifier (namespace prefix), \
                     got \"{}\". Dots, braces, spaces, and other special characters are not allowed.",
                    trimmed
                ),
            ));
        }
    }

    Ok(Some(trimmed.to_string()))
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p kirin-derive-chumsky`
Expected: All pass (new function is not called yet).

**Step 3: Commit**

```bash
git add crates/kirin-derive-chumsky/src/codegen/helpers.rs
git commit -m "feat(derive-chumsky): add namespace_for_wrapper validation helper"
```

---

### Task 3: Modify Parser Codegen for Namespace Prefix

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/generate.rs`

**Step 1: Pass stmt to build_wrapper_parser and add namespace prefix**

In `crates/kirin-derive-chumsky/src/codegen/parser/generate.rs`:

1. Change the `build_statement_parser` call at line 114 to also pass `stmt`:
   ```rust
   if let Some(wrapper) = &stmt.wraps {
       return self.build_wrapper_parser(ir_input, stmt, ast_name, variant, wrapper, crate_path);
   }
   ```

2. Update `build_wrapper_parser` signature and body to accept `stmt` and use namespace:
   ```rust
   fn build_wrapper_parser(
       &self,
       ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
       stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
       ast_name: &syn::Ident,
       variant: Option<&syn::Ident>,
       wrapper: &kirin_derive_toolkit::ir::fields::Wrapper,
       crate_path: &syn::Path,
   ) -> syn::Result<TokenStream> {
       let wrapped_ty = &wrapper.ty;
       let namespace = crate::codegen::namespace_for_wrapper(ir_input, stmt)?;

       let constructor = match variant {
           Some(v) => quote! { #ast_name::#v },
           None => quote! { #ast_name },
       };

       let type_output = quote! { __TypeOutput };
       let language_output = quote! { __LanguageOutput };
       let return_type =
           self.build_ast_type_reference(ir_input, ast_name, &type_output, &language_output);

       let inner_parser = quote! {
           <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::recursive_parser::<I, __TypeOutput, __LanguageOutput>(language.clone())
       };

       let parser = if let Some(ns) = namespace {
           quote! {
               {
                   use #crate_path::Token;
                   #crate_path::chumsky::prelude::just(Token::Identifier(#ns))
                       .then_ignore(#crate_path::chumsky::prelude::just(Token::Dot))
                       .ignore_then(#inner_parser)
                       .map(|inner| -> #return_type { #constructor(inner) })
               }
           }
       } else {
           quote! {
               #inner_parser
                   .map(|inner| -> #return_type { #constructor(inner) })
           }
       };

       Ok(parser)
   }
   ```

**Step 2: Run tests**

Run: `cargo nextest run -p kirin-derive-chumsky`
Expected: All pass.

**Step 3: Commit**

```bash
git add crates/kirin-derive-chumsky/src/codegen/parser/generate.rs
git commit -m "feat(derive-chumsky): add namespace prefix to wrapper parser codegen"
```

---

### Task 4: Modify PrettyPrint Codegen for Namespace Prefix

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/helpers.rs` (update `generate_enum_match` signature)
- Modify: `crates/kirin-derive-chumsky/src/codegen/pretty_print/statement.rs`
- Modify: `crates/kirin-derive-chumsky/src/codegen/emit_ir/generate.rs` (if it uses `generate_enum_match`)

**Step 1: Update `generate_enum_match` to pass `stmt` to wrapper handler**

In `crates/kirin-derive-chumsky/src/codegen/helpers.rs`, change the `wrapper_handler` closure type in `generate_enum_match`:

```rust
pub(crate) fn generate_enum_match<L: Layout, F, G>(
    type_name: &syn::Ident,
    data: &kirin_derive_toolkit::ir::DataEnum<L>,
    wrapper_handler: F,
    regular_handler: G,
    marker_handler: Option<TokenStream>,
) -> TokenStream
where
    F: Fn(&syn::Ident, &kirin_derive_toolkit::ir::fields::Wrapper, &kirin_derive_toolkit::ir::Statement<L>) -> TokenStream,
    G: Fn(&syn::Ident, &kirin_derive_toolkit::ir::Statement<L>) -> TokenStream,
{
    let arms: Vec<TokenStream> = data
        .iter_variants()
        .map(|variant| match variant {
            VariantRef::Wrapper { name, wrapper, stmt } => {
                let body = wrapper_handler(name, wrapper, stmt);
                quote! { #type_name::#name(inner) => { #body } }
            }
            VariantRef::Regular { name, stmt } => regular_handler(name, stmt),
        })
        .collect();
    // ... rest unchanged
```

**Step 2: Update all callers of `generate_enum_match`**

Search all callers and add the `_stmt` parameter to their wrapper closures:

In `crates/kirin-derive-chumsky/src/codegen/pretty_print/statement.rs` line 233:
```rust
|name, _wrapper, stmt| {
    let namespace = crate::codegen::namespace_for_wrapper(ir_input, stmt).ok().flatten();
    if let Some(ns) = namespace {
        let prefix = format!("{}.", ns);
        quote! {
            doc.text(#prefix) + #prettyless_path::PrettyPrint::pretty_print(inner, doc)
        }
    } else {
        quote! {
            #prettyless_path::PrettyPrint::pretty_print(inner, doc)
        }
    }
},
```

In `crates/kirin-derive-chumsky/src/codegen/emit_ir/generate.rs` (check if it calls `generate_enum_match` — update its wrapper closure to accept the extra `_stmt` param).

Search for other callers:

Run: `grep -rn "generate_enum_match" crates/kirin-derive-chumsky/src/` to find all call sites.

**Step 3: Run tests**

Run: `cargo nextest run -p kirin-derive-chumsky`
Expected: All pass.

**Step 4: Commit**

```bash
git add crates/kirin-derive-chumsky/src/codegen/
git commit -m "feat(derive-chumsky): add namespace prefix to wrapper pretty-print codegen"
```

---

### Task 5: Integration Test — Namespace Roundtrip

**Files:**
- Modify: `crates/kirin-test-languages/src/composite_language.rs` (add namespace to one variant)
- Create: `tests/namespace.rs`

**Step 1: Add namespace to CompositeLanguage**

In `crates/kirin-test-languages/src/composite_language.rs`, add `#[chumsky(format = "arith")]` on the `Arith` variant:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
#[wraps]
pub enum CompositeLanguage {
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "arith")
    )]
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}
```

**Step 2: Write a roundtrip test**

Create `tests/namespace.rs`:

```rust
use kirin::prelude::*;
use kirin::parsers::{EmitContext, EmitIR, parse_ast};
use kirin::pretty::{Config, PrettyPrint as _};
use kirin_test_languages::CompositeLanguage;

/// Test that namespace-prefixed parsing and printing round-trips.
#[test]
fn test_namespace_roundtrip_arith() {
    let mut stage: StageInfo<CompositeLanguage> = StageInfo::default();

    // Create operand SSAs
    let ssa_a = stage
        .ssa()
        .name("a".to_string())
        .ty(kirin_arith::ArithType::I64)
        .kind(SSAKind::Test)
        .new();
    let ssa_b = stage
        .ssa()
        .name("b".to_string())
        .ty(kirin_arith::ArithType::I64)
        .kind(SSAKind::Test)
        .new();

    // Parse with namespace prefix
    let input = "%res = arith.add %a, %b -> f64";
    let ast = parse_ast::<CompositeLanguage>(input).expect("parse failed");

    // Emit
    let mut emit_ctx = EmitContext::new(&mut stage);
    emit_ctx.register_ssa("a".to_string(), ssa_a);
    emit_ctx.register_ssa("b".to_string(), ssa_b);
    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&stage).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Pretty print
    let doc = Document::new(Config::default(), &stage);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc.render_fmt(80, &mut output).expect("render failed");

    // Roundtrip should match
    assert_eq!(output.trim(), input);
}
```

**Note:** The exact field types and format strings for `Arith<ArithType>` need to match what `kirin-arith` defines. Check `crates/kirin-arith/src/lib.rs` to verify the correct format. The test above is a template — adjust the input string and assertions to match the actual `Arith` format strings. If `Arith`'s `Add` format is `"{2:name} = add {0}, {1} -> {2:type}"`, then with namespace it becomes `"%res = arith.add %a, %b -> f64"`.

**Step 3: Run the test**

Run: `cargo nextest run -E 'test(test_namespace)'`
Expected: PASS — the namespace prefix roundtrips correctly.

**Step 4: Commit**

```bash
git add crates/kirin-test-languages/src/composite_language.rs tests/namespace.rs
git commit -m "feat: add namespace prefix integration test for dialect composition"
```

---

### Task 6: Update Snapshot Tests

**Files:**
- Modify: Various snapshot files under `crates/kirin-derive-chumsky/src/`

**Step 1: Run all snapshot tests and review changes**

Run: `cargo nextest run --workspace`
Then: `cargo insta review`

Review each snapshot change — format string snapshots should be unaffected (they don't test wrapper variants). If any snapshots changed due to the lexer change (removing `.` from identifiers), accept them.

**Step 2: Run doctests**

Run: `cargo test --doc --workspace`
Expected: All pass.

**Step 3: Commit snapshot updates**

```bash
git add -A crates/kirin-derive-chumsky/src/**/snapshots/
git commit -m "test: update snapshots for namespace prefix feature"
```

---

### Task 7: Update Design Doc Status

**Files:**
- Modify: `docs/plans/2026-03-05-namespace-prefix-design.md`

**Step 1: Update status and add lexer prerequisite note**

Change status from `Approved` to `Implemented`. Add a note about the lexer change (Token::Dot addition) being a prerequisite.

**Step 2: Commit**

```bash
git add docs/plans/2026-03-05-namespace-prefix-design.md
git commit -m "docs: mark namespace prefix design as implemented"
```
