# RFC 0003 Task 5: Projection Parse Codegen

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate parser code for format-string body projections (`{body:ports}`, `{body:body}`, etc.) so the derive macro can parse individual structural parts of DiGraph/UnGraph/Block/Region fields and reconstruct the full IR type.

**Architecture:** Each body projection generates a different component parser call. Multiple projections of the same field produce multiple AST variables. A new reconstruction step in the AST constructor combines parsed pieces into the full AST type (e.g., `DiGraph<'t, ...>`) before emit. The emit codegen is unchanged — it still calls `.emit_with()` on the reconstructed AST.

**Key design decision:** The AST type for a projected field remains the same (`DiGraph<'t, TypeOutput, LanguageOutput>`). The projection pieces are parsed into intermediate types, then assembled into the expected AST type in the constructor. This avoids changes to emit codegen.

---

## Prerequisites

- RFC 0003 Tasks 1-4 complete (format parser, validation, HasSignature, component parsers)
- RFC 0003 Task 6 complete (print codegen for projections)
- All 1139 tests passing on `rust` branch

## Key files

| File | Role |
|------|------|
| `crates/kirin-derive-chumsky/src/field_kind.rs` | `parser_expr()` — generates parser call per projection |
| `crates/kirin-derive-chumsky/src/codegen/parser/chain.rs` | `ast_constructor()` — builds AST from parsed variables |
| `crates/kirin-derive-chumsky/src/validation.rs` | `FieldOccurrence` — tracks multiple projections per field |
| `crates/kirin-chumsky/src/parsers/graphs.rs` | Component parsers: `port_list()`, `capture_list()`, `yield_type_list()`, `digraph_body_statements()`, `ungraph_body_statements()` |
| `crates/kirin-chumsky/src/parsers/blocks.rs` | Component parsers: `block_argument_list_bare()`, `block_body_statements()`, `region_body()` |
| `crates/kirin-chumsky/src/ast/graphs.rs` | `DiGraph<'t, ...>`, `UnGraph<'t, ...>` AST types with `emit_with()` |
| `crates/kirin-chumsky/src/ast/blocks.rs` | `Block<'t, ...>`, `Region<'t, ...>` AST types with `emit_with()` |

## Background: Current AST types

```rust
// DiGraph AST (ast/graphs.rs)
pub struct DiGraph<'src, TypeOutput, StmtOutput> {
    pub header: Spanned<GraphHeader<'src, TypeOutput>>,
    pub statements: Vec<Spanned<StmtOutput>>,
    pub yields: Vec<Spanned<&'src str>>,
}
pub struct GraphHeader<'src, TypeOutput> {
    pub name: Spanned<&'src str>,
    pub ports: Vec<Spanned<BlockArgument<'src, TypeOutput>>>,
    pub captures: Vec<Spanned<BlockArgument<'src, TypeOutput>>>,
}

// UnGraph AST (ast/graphs.rs)
pub struct UnGraph<'src, TypeOutput, StmtOutput> {
    pub header: Spanned<GraphHeader<'src, TypeOutput>>,
    pub statements: Vec<UnGraphStatement<'src, StmtOutput>>,
}

// Block AST (ast/blocks.rs)
pub struct Block<'src, TypeOutput, StmtOutput> {
    pub header: Spanned<BlockHeader<'src, TypeOutput>>,
    pub statements: Vec<Spanned<StmtOutput>>,
}

// Region AST (ast/blocks.rs)
pub struct Region<'src, TypeOutput, StmtOutput> {
    pub blocks: Vec<Spanned<Block<'src, TypeOutput, StmtOutput>>>,
}
```

The emit codegen calls `var.emit_with(ctx, emit_language_output)?` on each body field. This stays unchanged.

## Background: Component parser return types

| Parser | Returns | Used for |
|--------|---------|----------|
| `port_list::<_, T>()` | `Vec<Spanned<BlockArgument<'t, T::Output>>>` | `{field:ports}` |
| `capture_list::<_, T>()` | `Vec<Spanned<BlockArgument<'t, T::Output>>>` | `{field:captures}` |
| `yield_type_list::<_, T>()` | `Vec<Spanned<T::Output>>` | `{field:yields}` |
| `digraph_body_statements(lang)` | `(Vec<Spanned<S>>, Vec<Spanned<&'t str>>)` | `{field:body}` on DiGraph |
| `ungraph_body_statements(lang)` | `Vec<UnGraphStatement<'t, S>>` | `{field:body}` on UnGraph |
| `block_argument_list_bare::<_, T>()` | `Vec<Spanned<BlockArgument<'t, T::Output>>>` | `{field:args}` |
| `block_body_statements(lang)` | `Vec<Spanned<S>>` | `{field:body}` on Block |
| `region_body::<_, T, _>(lang)` | `Vec<Spanned<Block<'t, T::Output, S>>>` | `{field:body}` on Region |

## Background: How FieldOccurrence works

Each `{field:option}` in the format string creates a `FieldOccurrence`:
```rust
pub struct FieldOccurrence<'a> {
    pub field: &'a FieldInfo<ChumskyLayout>,
    pub option: FormatOption,      // Default, Name, Type, Body(proj), Function(proj)
    pub var_name: syn::Ident,      // Unique: body_ports, body_body, body_yields, etc.
}
```

Multiple projections of the same field (e.g., `{body:ports}`, `{body:body}`, `{body:yields}`) produce 3 occurrences with different `var_name`s but the same `field.index`. The `chain.rs:build_field_value()` collects these via `field_occurrences.get(&field.index)`.

---

## Task 5.1: Parser expression generation

**File:** `crates/kirin-derive-chumsky/src/field_kind.rs`

- [ ] **Step 1: Read current `parser_expr()` function**

Note how DiGraph/UnGraph/Block/Region categories currently generate full parser calls. The `opt` parameter is not matched — all options produce the same parser.

- [ ] **Step 2: Add projection-aware parser expressions for DiGraph**

Replace the flat `FieldCategory::DiGraph` arm with a nested match:

```rust
FieldCategory::DiGraph => match opt {
    FormatOption::Default => {
        quote! { #crate_path::digraph::<_, #ir_type, _>(language.clone()) }
    }
    FormatOption::Body(BodyProjection::Ports) => {
        quote! { #crate_path::port_list::<_, #ir_type>() }
    }
    FormatOption::Body(BodyProjection::Captures) => {
        quote! { #crate_path::capture_list::<_, #ir_type>() }
    }
    FormatOption::Body(BodyProjection::Yields) => {
        quote! { #crate_path::yield_type_list::<_, #ir_type>() }
    }
    FormatOption::Body(BodyProjection::Body) => {
        quote! { #crate_path::digraph_body_statements(language.clone()) }
    }
    FormatOption::Body(BodyProjection::Args) => {
        unreachable!("validation prevents :args on DiGraph")
    }
    _ => unreachable!("validation prevents Name/Type/Function on DiGraph")
},
```

- [ ] **Step 3: Add projection-aware parser expressions for UnGraph**

```rust
FieldCategory::UnGraph => match opt {
    FormatOption::Default => {
        quote! { #crate_path::ungraph::<_, #ir_type, _>(language.clone()) }
    }
    FormatOption::Body(BodyProjection::Ports) => {
        quote! { #crate_path::port_list::<_, #ir_type>() }
    }
    FormatOption::Body(BodyProjection::Captures) => {
        quote! { #crate_path::capture_list::<_, #ir_type>() }
    }
    FormatOption::Body(BodyProjection::Body) => {
        quote! { #crate_path::ungraph_body_statements(language.clone()) }
    }
    FormatOption::Body(BodyProjection::Yields) => {
        unreachable!("validation prevents :yields on UnGraph")
    }
    FormatOption::Body(BodyProjection::Args) => {
        unreachable!("validation prevents :args on UnGraph")
    }
    _ => unreachable!()
},
```

- [ ] **Step 4: Add projection-aware parser expressions for Block**

```rust
FieldCategory::Block => match opt {
    FormatOption::Default => {
        quote! { #crate_path::block::<_, #ir_type, _>(language.clone()) }
    }
    FormatOption::Body(BodyProjection::Args) => {
        quote! { #crate_path::block_argument_list_bare::<_, #ir_type>() }
    }
    FormatOption::Body(BodyProjection::Body) => {
        quote! { #crate_path::block_body_statements(language.clone()) }
    }
    _ => unreachable!("validation prevents other projections on Block")
},
```

- [ ] **Step 5: Add projection-aware parser expressions for Region**

```rust
FieldCategory::Region => match opt {
    FormatOption::Default => {
        quote! { #crate_path::region::<_, #ir_type, _>(language.clone()) }
    }
    FormatOption::Body(BodyProjection::Body) => {
        quote! { #crate_path::region_body::<_, #ir_type, _>(language.clone()) }
    }
    _ => unreachable!("validation prevents other projections on Region")
},
```

- [ ] **Step 6: Build and verify**

```bash
cargo build -p kirin-derive-chumsky
```

- [ ] **Step 7: Commit**

```
feat(derive-chumsky): generate projection-specific parser expressions for body fields
```

---

## Task 5.2: AST type for projected fields

**Problem:** When a DiGraph field has projections (`{body:ports}`, `{body:body}`, `{body:yields}`), each projection parses a different type. But the AST struct expects a single `DiGraph<'t, ...>` field.

**Solution:** When the field has projections, change the AST field type to hold a **reconstruction tuple** instead of the full type. Then in the constructor, assemble the tuple into the final AST type.

**File:** `crates/kirin-derive-chumsky/src/field_kind.rs` — `ast_type()`

- [ ] **Step 1: Determine if a field uses projections**

Add a helper function:

```rust
/// Returns true if any occurrence of this field uses a body projection.
pub fn has_body_projections(
    field: &FieldInfo<impl Layout>,
    format: &Format<'_>,
) -> bool {
    for elem in format.elements() {
        if let FormatElement::Field(name, FormatOption::Body(_)) = elem {
            // Check if this field name matches
            if field_matches_name(field, name) {
                return true;
            }
        }
    }
    false
}
```

- [ ] **Step 2: Don't change ast_type for projected fields**

Actually, the AST type stays the same (`DiGraph<'t, ...>`). The **constructor** in `chain.rs` will assemble the projected pieces. The individual projection parser outputs are typed as intermediate values, not as AST fields. The parser chain (`chain.rs`) already handles this — each `{field:proj}` becomes a separate parsed variable, and `build_field_value` combines them.

So no changes to `ast_type()`. Instead, focus on the constructor.

- [ ] **Step 3: Commit (if any changes)**

This step may produce no changes — the key work is in Task 5.3.

---

## Task 5.3: AST reconstruction from projection pieces

**File:** `crates/kirin-derive-chumsky/src/codegen/parser/chain.rs`

This is the most complex part. When `build_field_value()` encounters a field with body projection occurrences, it must reconstruct the full AST type from the parsed pieces.

- [ ] **Step 1: Read `build_field_value()` (lines 346-407)**

Understand how it currently handles multiple occurrences (Name + Type → `construct_from_name_and_type`).

- [ ] **Step 2: Add projection reconstruction for DiGraph**

When a DiGraph field has body projection occurrences, generate code that reconstructs a `DiGraph` AST from the pieces:

```rust
fn build_projected_digraph_value(
    &self,
    field: &FieldInfo<ChumskyLayout>,
    occurrences: &[&FieldOccurrence],
    crate_path: &syn::Path,
) -> TokenStream {
    // Find each projection's variable
    let ports_var = occurrences.iter()
        .find(|o| matches!(o.option, FormatOption::Body(BodyProjection::Ports)))
        .map(|o| &o.var_name);
    let captures_var = occurrences.iter()
        .find(|o| matches!(o.option, FormatOption::Body(BodyProjection::Captures)))
        .map(|o| &o.var_name);
    let yields_var = occurrences.iter()
        .find(|o| matches!(o.option, FormatOption::Body(BodyProjection::Yields)))
        .map(|o| &o.var_name);
    let body_var = occurrences.iter()
        .find(|o| matches!(o.option, FormatOption::Body(BodyProjection::Body)))
        .map(|o| &o.var_name);

    // Reconstruct DiGraph AST
    let ports_expr = ports_var
        .map(|v| quote! { #v })
        .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });
    let captures_expr = captures_var
        .map(|v| quote! { #v })
        .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });

    // digraph_body_statements returns (Vec<Spanned<S>>, Vec<Spanned<&str>>)
    let (stmts_expr, yields_expr) = if let Some(body) = body_var {
        (quote! { #body.0 }, quote! { #body.1 })
    } else {
        (quote! { ::std::vec::Vec::new() }, quote! { ::std::vec::Vec::new() })
    };

    // Generate a synthetic graph name (^projected0)
    // The emit_with phase will assign the real graph name.
    quote! {
        #crate_path::DiGraph {
            header: #crate_path::Spanned {
                value: #crate_path::GraphHeader {
                    name: #crate_path::Spanned { value: "projected", span: Default::default() },
                    ports: #ports_expr,
                    captures: #captures_expr,
                },
                span: Default::default(),
            },
            statements: #stmts_expr,
            yields: #yields_expr,
        }
    }
}
```

**Important:** Check that `crate_path` correctly resolves `DiGraph`, `GraphHeader`, `Spanned` — these are in `kirin_chumsky::ast`.

- [ ] **Step 3: Add projection reconstruction for UnGraph**

Similar pattern. `ungraph_body_statements` returns `Vec<UnGraphStatement<'t, S>>`.

```rust
fn build_projected_ungraph_value(...) -> TokenStream {
    // ports, captures from projections
    // statements from :body
    quote! {
        #crate_path::UnGraph {
            header: #crate_path::Spanned {
                value: #crate_path::GraphHeader {
                    name: #crate_path::Spanned { value: "projected", span: Default::default() },
                    ports: #ports_expr,
                    captures: #captures_expr,
                },
                span: Default::default(),
            },
            statements: #stmts_expr,
        }
    }
}
```

- [ ] **Step 4: Add projection reconstruction for Block**

```rust
fn build_projected_block_value(...) -> TokenStream {
    // args from :args, statements from :body
    quote! {
        #crate_path::Spanned {
            value: #crate_path::Block {
                header: #crate_path::Spanned {
                    value: #crate_path::BlockHeader {
                        label: #crate_path::BlockLabel { name: "projected", span: Default::default() },
                        arguments: #args_expr,
                    },
                    span: Default::default(),
                },
                statements: #stmts_expr,
            },
            span: Default::default(),
        }
    }
}
```

- [ ] **Step 5: Add projection reconstruction for Region**

```rust
fn build_projected_region_value(...) -> TokenStream {
    // blocks from :body (region_body returns Vec<Spanned<Block>>)
    quote! {
        #crate_path::Region {
            blocks: #body_var,
        }
    }
}
```

- [ ] **Step 6: Wire into build_field_value**

In `build_field_value()`, add detection for projection occurrences:

```rust
fn build_field_value(&self, field, field_occurrences, crate_path, result_index) -> TokenStream {
    let occs = field_occurrences.get(&field.index);
    match occs {
        Some(occs) if occs.iter().any(|o| matches!(o.option, FormatOption::Body(_))) => {
            // Projected field — reconstruct from pieces
            match field.category() {
                FieldCategory::DiGraph => self.build_projected_digraph_value(field, occs, crate_path),
                FieldCategory::UnGraph => self.build_projected_ungraph_value(field, occs, crate_path),
                FieldCategory::Block => self.build_projected_block_value(field, occs, crate_path),
                FieldCategory::Region => self.build_projected_region_value(field, occs, crate_path),
                _ => unreachable!("body projections only valid on body field types"),
            }
        }
        // ... existing cases for Name/Type/Default ...
    }
}
```

- [ ] **Step 7: Build and test**

```bash
cargo build -p kirin-derive-chumsky
cargo nextest run -p kirin-derive-chumsky
```

- [ ] **Step 8: Commit**

```
feat(derive-chumsky): reconstruct body field AST from projection pieces in parser chain
```

---

## Task 5.4: Handle {:name} pseudo-field parsing

The `{:name}` projection parses `@symbol_name` from the text. It's not a real struct field — "function" is a pseudo-field that provides context.

**File:** `crates/kirin-derive-chumsky/src/codegen/parser/chain.rs`

- [ ] **Step 1: Detect function projection in build_parser_chain**

When `FormatElement::Field("function", FormatOption::Function(Name))` is encountered, generate a `symbol()` parser that captures the function name:

```rust
FormatElement::Field(name, opt) if *name == "function" => {
    // Pseudo-field: parse @symbol and store as context
    match opt {
        FormatOption::Function(FunctionProjection::Name) => {
            parser_parts.push(ParserPart::Field(
                quote! { #crate_path::symbol() }
            ));
        }
        _ => unreachable!("only :name projection on function pseudo-field")
    }
}
```

The parsed `SymbolName` is captured but NOT assigned to any struct field. It's available to the function text parser (Task 7) for function name extraction.

- [ ] **Step 2: Skip function pseudo-field in ast_constructor**

In `ast_constructor()`, when a variable comes from a "function" pseudo-field, don't assign it to any struct field. Just let it be parsed and discarded at the AST level (the function text parser handles it separately).

- [ ] **Step 3: Build and test**

```bash
cargo build --workspace
cargo nextest run --workspace
```

- [ ] **Step 4: Commit**

```
feat(derive-chumsky): handle {:name} pseudo-field in parser codegen
```

---

## Task 5.5: Snapshot tests

**File:** `crates/kirin-derive-chumsky/src/codegen/parser/` (existing snapshot tests)

- [ ] **Step 1: Add snapshot test for projected DiGraph format**

```rust
#[test]
fn test_projected_digraph_parser() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "fn {:name}({body:ports}) -> {body:yields} { {body:body} }")]
        struct MyFunction {
            pub body: DiGraph,
        }
    };
    insta::assert_snapshot!(generate_parser_code(input));
}
```

- [ ] **Step 2: Add snapshot test for projected Block format**

```rust
#[test]
fn test_projected_block_parser() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[chumsky(format = "fn {:name}({body:args}) { {body:body} }")]
        struct BlockFunction {
            pub body: Block,
        }
    };
    insta::assert_snapshot!(generate_parser_code(input));
}
```

- [ ] **Step 3: Run and review snapshots**

```bash
cargo insta test -p kirin-derive-chumsky
cargo insta review
```

- [ ] **Step 4: Commit**

```
test(derive-chumsky): add snapshot tests for projection parser codegen
```

---

## Task 5.6: End-to-end verification

- [ ] **Step 1: Full workspace build**

```bash
cargo build --workspace
```

- [ ] **Step 2: Full test suite**

```bash
cargo nextest run --workspace
```

All 1139+ tests must pass with no regressions.

- [ ] **Step 3: Commit any fixes**

---

## Dependency Graph

```
Task 5.1 (parser_expr) ── Task 5.3 (AST reconstruction) ── Task 5.5 (snapshots)
                                                                  │
Task 5.2 (ast_type — likely no-op)                         Task 5.6 (e2e verify)
                                                                  │
Task 5.4 ({:name})  ──────────────────────────────────────┘
```

## Risk Assessment

**HIGH RISK:** Task 5.3 (AST reconstruction). The generated code must produce valid `DiGraph`/`UnGraph`/`Block`/`Region` AST values from projection pieces. Key risks:
- Span information for synthetic headers (`Default::default()`)
- Missing projections (e.g., no `{body:captures}` → default to empty vec)
- Type inference in generated code (AST generics must resolve correctly)

**MEDIUM RISK:** Task 5.1 (parser expressions). Component parsers have different signatures — must ensure type annotations are correct in generated code.

**LOW RISK:** Tasks 5.2, 5.4, 5.5, 5.6.
