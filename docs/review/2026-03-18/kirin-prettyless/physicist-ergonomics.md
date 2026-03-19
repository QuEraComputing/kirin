# Physicist -- Ergonomics/DX Review: kirin-prettyless

## Repetition & Boilerplate

### 1. Manual PrettyPrint impls for custom types are verbose
When implementing `PrettyPrint` for a custom type like `ArithType`, the signature is:
```rust
fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
    &self,
    doc: &'a Document<'a, L>,
    _namespace: &[&str],
) -> ArenaDoc<'a>
where
    L::Type: std::fmt::Display,
```
This is 6 lines of signature for what amounts to `doc.text(self.to_string())`. The `L: Dialect + PrettyPrint` bound and `L::Type: Display` bound are required by the trait but irrelevant to simple types that just render themselves. The `_namespace` parameter is unused for leaf types.

For comparison, `Display` achieves the same for text output with 4 lines.

### 2. PrettyPrint for custom types duplicates Display
Most custom type `PrettyPrint` impls just call `doc.text(self.to_string())`, which means the user writes both `Display` and `PrettyPrint`. A blanket impl `PrettyPrint for T where T: Display` would eliminate this duplication for leaf types.

However, there may be cases where `PrettyPrint` and `Display` should differ (e.g., structured types that use document nesting). The blanket impl would prevent those customizations. A middle ground: a `#[derive(PrettyPrint)]` for simple types that delegates to `Display`, or a marker trait like `PrettyPrintViaDisplay`.

### 3. `sprint` and `render` API is clean
The `PrettyPrintExt` blanket impl provides:
- `node.sprint(&stage)` -- one-liner for string output
- `node.render(&stage).config(...).globals(...).to_string()` -- builder for options

This is well-designed. No repetition concerns.

## Lifetime Complexity

### PrettyPrint trait has one generic lifetime
```rust
fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
```
The `'a` ties the output document to the allocator. This is standard for arena-based document builders. Users implementing custom printers encounter this but it's a single, well-scoped lifetime.

### RenderBuilder has two lifetimes
```rust
struct RenderBuilder<'n, 's, N, L: Dialect>
```
Users never write this type -- they call `node.render(&stage)` and chain methods. Inference handles everything. Good.

### PipelinePrintExt hides all complexity
The `pipeline.sprint()` method (used in toy-lang `main.rs:49`) hides all document/stage/config machinery behind a single method call. Excellent.

## Concept Budget

### Use Case: "Pretty-print my dialect"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `#[derive(PrettyPrint)]` | kirin-derive-chumsky | Low |
| `#[chumsky(format = "...")]` (shared with parser) | derive attrs | Low (already learned for parser) |
| `sprint` / `render` API | kirin-prettyless | Low |
| `StageInfo<L>` context | kirin-ir | Low (already learned) |

**Total: ~4 concepts.** Minimal, since format strings are shared with parser derive.

### Use Case: "Pretty-print a pipeline"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `PipelinePrintExt::sprint` | kirin-prettyless | Low |
| `#[derive(RenderDispatch)]` on stage enum | kirin-derive-prettyless | Low |

**Total: ~2 concepts.** Excellent.

### Use Case: "Implement PrettyPrint for a custom type lattice"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `PrettyPrint` trait signature | kirin-prettyless | Medium |
| `Document<'a, L>` and `ArenaDoc<'a>` | kirin-prettyless | Medium |
| `doc.text(...)` and other document builders | prettyless crate | Medium |
| `L: Dialect + PrettyPrint` bound | kirin-prettyless | Low (just copy it) |
| `Display` impl (needed anyway) | std | Low |

**Total: ~5 concepts.** The document builder API (from the `prettyless` crate) is the new concept here.

## Toy Scenario Results

### Scenario: Print a parsed pipeline to stdout

```rust
let rendered = pipeline.sprint();
print!("{rendered}");
```

Two lines. Cannot be simpler. Excellent.

### Scenario: Print a single statement for debugging

```rust
let output = statement.sprint(&stage);
println!("{output}");
```

Also two lines. The `stage` context is needed for SSA name resolution. This makes sense.

### Scenario: Implement PrettyPrint for PulseType

```rust
impl PrettyPrint for PulseType {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(self.to_string())
    }
}
```

**What worked well:**
- The implementation is trivial once you know the pattern
- `doc.text(...)` is obvious

**What was confusing:**
- Why does this trait need `L` and `namespace`? For my simple enum type, they're irrelevant
- The `where L::Type: Display` bound is inherited from the trait but never used in my impl
- I already implemented `Display` for PulseType; having to also write PrettyPrint feels redundant

### Scenario: Configure output width

```rust
let output = statement.render(&stage)
    .config(Config::default().with_width(40))
    .to_string()?;
```

Clean builder API. The `?` is needed because rendering can fail (e.g., missing SSA info). This makes sense.

## Summary

- [P2] [confirmed] Manual `PrettyPrint` impls for simple types duplicate `Display` with extra boilerplate. `kirin-arith/src/types/arith_type.rs:104-115`
- [P2] [likely] A blanket impl or marker trait (`PrettyPrintViaDisplay`) for types where `PrettyPrint` just delegates to `Display` would eliminate the most common case of manual impl.
- [P3] [confirmed] `PrettyPrint` trait signature exposes `L`, `namespace`, and `where L::Type: Display` that are irrelevant for leaf type impls.
- [P3] [informational] `sprint` and `render` builder API is clean and well-layered (shorthand -> builder -> full control).
- [P3] [informational] `PipelinePrintExt::sprint` is a single method call that hides all complexity. Excellent for the common case.
