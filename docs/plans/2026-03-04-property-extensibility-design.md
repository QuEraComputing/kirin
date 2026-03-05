# PropertyKind Extensibility Design

**Date**: 2026-03-04
**Status**: Approved

## Problem Statement

`PropertyKind` in `kirin-derive-core/src/generators/property/context.rs` is a closed 4-variant enum (`Constant`, `Pure`, `Speculatable`, `Terminator`). A downstream crate wanting to implement `#[derive(IsQuantum)]` driven by `#[quantum]` cannot do so without modifying kirin-derive-core. The property system should be open for extension without touching core.

## Design Goal

A downstream `#[derive(IsQuantum)]` proc-macro should be ~10 lines of code:

```rust
#[proc_macro_derive(IsQuantum, attributes(kirin, wraps, quantum))]
pub fn derive_is_quantum(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let mut derive = DeriveProperty::bare_attr(
        "quantum",                     // bare attribute name
        "::my_quantum_dialect",        // default crate path
        "IsQuantum",                   // trait name
        "is_quantum",                  // trait method
        "bool",                        // return type
    );
    match derive.emit(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}
```

No trait implementation required. No Layout extension. No custom Scan/Emit. Just a constructor call.

## Current Architecture Summary

### How properties work today

1. **Attribute parsing** (`ir/attrs.rs`): `KirinStructOptions`, `KirinEnumOptions`, and `StatementOptions` each have hardcoded `bool` fields (`constant`, `pure`, `speculatable`, `terminator`) parsed via darling.

2. **PropertyKind enum** (`generators/property/context.rs`): Maps each variant to its corresponding bool field via `global_value()` and `statement_value()`.

3. **DeriveProperty** (`context.rs`): Holds `kind: PropertyKind`, trait metadata (path, method, value type), plus scan/emit state.

4. **Scan** (`scan.rs`): Reads the bool from `PropertyKind::global_value()`, stores in `InputContext`. Runs cross-property validation (constant requires pure, speculatable requires pure).

5. **Emit** (`emit.rs`): Generates match arms returning `global_value || statement_value` per variant, or delegates to wrapper's trait method.

6. **Proc-macro entry** (`kirin-derive/src/lib.rs`): Each property derive is declared with a `PropertyConfig` and registered via `derive_property_macro!`. The `Dialect` derive runs all 4 property configs.

### How Layout extension works today

The `Layout` trait has 4 associated types (`StatementExtra`, `ExtraGlobalAttrs`, `ExtraStatementAttrs`, `ExtraFieldAttrs`). `EvalCallLayout` in `kirin-derive-interpreter` overrides global/statement attrs to add `callable: bool`, then implements full `Scan`/`Emit` from scratch. This is the mechanism for **complex** derives that need custom code generation.

### How HasParser works (complex derive)

`kirin-chumsky-derive` does NOT use `kirin-derive-core`'s `Scan`/`Emit` framework. It delegates to `kirin-chumsky-format` with its own pipeline and `#[chumsky(...)]` attributes. Complex derives are already extensible by writing independent pipelines.

### kirin-derive-dialect (dead crate)

Not in the workspace. Duplicates generators from `kirin-derive-core`. Files have diverged. Should be deleted.

## Proposed Design

### Principle: Three tiers of extensibility

| Tier | Mechanism | Downstream effort | Example |
|------|-----------|-------------------|---------|
| **Easy** | `DeriveProperty::bare_attr("name", ...)` | ~10 lines, no trait impl | `#[derive(IsQuantum)]` with `#[quantum]` |
| **Middle** | Implement `PropertyValueReader`, pass to `DeriveProperty::with_reader(reader, ...)` | ~30 lines + trait impl | Custom validation, computed values, multi-attribute logic |
| **Full** | Independent pipeline (own Scan/Emit or fully custom) | Full crate | `#[derive(HasParser)]`, `#[derive(CallSemantics)]` |

The `PropertyValueReader` trait and `BareAttrReader` struct are both **public**, giving downstream developers an escape hatch for custom logic while still reusing `DeriveProperty`'s emit machinery.

### Public trait: `PropertyValueReader`

```rust
// In kirin-derive-core/src/generators/property/context.rs

/// Reads a boolean property value from derive input attributes.
///
/// Built-in properties read from `#[kirin(...)]` darling-parsed fields.
/// Downstream properties typically use `BareAttrReader` (via `DeriveProperty::bare_attr()`),
/// but can implement this trait directly for custom logic (validation, computed values, etc.).
pub trait PropertyValueReader {
    /// Read the global (type-level) property value.
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool;

    /// Read the per-statement (variant-level) property value.
    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool;

    /// Optional cross-property validation. Default: no validation.
    fn validate(&self, _input: &ir::Input<StandardLayout>) -> darling::Result<()> {
        Ok(())
    }
}
```

### Built-in reader: `PropertyKind` (unchanged externally)

The existing 4-variant enum implements `PropertyValueReader` internally, reading from darling-parsed `#[kirin(...)]` fields:

```rust
impl PropertyValueReader for PropertyKind {
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => input.attrs.constant,
            PropertyKind::Pure => input.attrs.pure,
            PropertyKind::Speculatable => input.attrs.speculatable,
            PropertyKind::Terminator => input.attrs.terminator,
        }
    }

    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool {
        match self {
            PropertyKind::Constant => statement.attrs.constant,
            PropertyKind::Pure => statement.attrs.pure,
            PropertyKind::Speculatable => statement.attrs.speculatable,
            PropertyKind::Terminator => statement.attrs.terminator,
        }
    }

    fn validate(&self, input: &ir::Input<StandardLayout>) -> darling::Result<()> {
        match self {
            PropertyKind::Constant => validate_constant_pure(input),
            PropertyKind::Speculatable => validate_speculatable_pure(input),
            _ => Ok(()),
        }
    }
}
```

### Bare attribute reader: `BareAttrReader` (public)

```rust
/// Reads a bare helper attribute (e.g., `#[quantum]`) from struct/variant attrs.
///
/// This is the reader used internally by `DeriveProperty::bare_attr()`.
/// It is also available directly for use with `DeriveProperty::with_reader()`
/// if you need to compose it with other logic.
pub struct BareAttrReader {
    attr_name: String,
}

impl BareAttrReader {
    pub fn new(attr_name: impl Into<String>) -> Self {
        Self { attr_name: attr_name.into() }
    }
}

impl PropertyValueReader for BareAttrReader {
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool {
        input.raw_attrs.iter().any(|a| a.path().is_ident(&self.attr_name))
    }

    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool {
        statement.raw_attrs.iter().any(|a| a.path().is_ident(&self.attr_name))
    }
}
```

### Revised `DeriveProperty` — three constructors

```rust
pub struct DeriveProperty {
    reader: Box<dyn PropertyValueReader>,  // private field
    pub default_crate_path: syn::Path,
    pub trait_path: syn::Path,
    pub trait_method: syn::Ident,
    pub value_type: syn::Type,
    pub(crate) input: Option<InputContext>,
    pub(crate) statements: HashMap<String, StatementInfo>,
}

impl DeriveProperty {
    /// Create a property derive for a built-in `#[kirin(...)]` property.
    /// Used by kirin-derive for IsConstant, IsPure, IsSpeculatable, IsTerminator.
    pub fn new(
        kind: PropertyKind,
        default_crate_path: impl Into<String>,
        trait_path: impl Into<String>,
        trait_method: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Self {
        Self {
            reader: Box::new(kind),
            default_crate_path: from_str(default_crate_path),
            trait_path: from_str(trait_path),
            trait_method: from_str(trait_method),
            value_type: from_str(value_type),
            input: None,
            statements: HashMap::new(),
        }
    }

    /// Create a property derive for a bare helper attribute.
    ///
    /// **Easy tier**: the primary extension point for downstream crates.
    /// The `attr_name` is the name of a bare attribute (e.g., "quantum"
    /// for `#[quantum]`) that can be placed on the type or individual
    /// variants to mark them as having this property.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut derive = DeriveProperty::bare_attr(
    ///     "quantum",
    ///     "::my_crate",
    ///     "IsQuantum",
    ///     "is_quantum",
    ///     "bool",
    /// );
    /// derive.emit(&ast)
    /// ```
    pub fn bare_attr(
        attr_name: impl Into<String>,
        default_crate_path: impl Into<String>,
        trait_path: impl Into<String>,
        trait_method: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Self {
        Self {
            reader: Box::new(BareAttrReader::new(attr_name)),
            default_crate_path: from_str(default_crate_path),
            trait_path: from_str(trait_path),
            trait_method: from_str(trait_method),
            value_type: from_str(value_type),
            input: None,
            statements: HashMap::new(),
        }
    }

    /// Create a property derive with a custom `PropertyValueReader`.
    ///
    /// **Middle tier**: for downstream crates that need custom logic
    /// (validation, computed values, multi-attribute combinations)
    /// while still reusing DeriveProperty's scan/emit machinery.
    ///
    /// # Example
    ///
    /// ```ignore
    /// struct MyReader;
    /// impl PropertyValueReader for MyReader {
    ///     fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool {
    ///         // custom logic
    ///     }
    ///     fn statement_value(&self, stmt: &ir::Statement<StandardLayout>) -> bool {
    ///         // custom logic
    ///     }
    ///     fn validate(&self, input: &ir::Input<StandardLayout>) -> darling::Result<()> {
    ///         // custom validation
    ///     }
    /// }
    ///
    /// let mut derive = DeriveProperty::with_reader(
    ///     MyReader,
    ///     "::my_crate",
    ///     "IsCustom",
    ///     "is_custom",
    ///     "bool",
    /// );
    /// derive.emit(&ast)
    /// ```
    pub fn with_reader(
        reader: impl PropertyValueReader + 'static,
        default_crate_path: impl Into<String>,
        trait_path: impl Into<String>,
        trait_method: impl Into<String>,
        value_type: impl Into<String>,
    ) -> Self {
        Self {
            reader: Box::new(reader),
            default_crate_path: from_str(default_crate_path),
            trait_path: from_str(trait_path),
            trait_method: from_str(trait_method),
            value_type: from_str(value_type),
            input: None,
            statements: HashMap::new(),
        }
    }
}
```

Three constructors for three tiers:
- `new(kind: PropertyKind, ...)` — built-in properties (backward compatible, same signature as today)
- `bare_attr(attr_name, ...)` — easy tier, no trait impl needed
- `with_reader(reader, ...)` — middle tier, custom `PropertyValueReader` impl

### Raw attribute storage in IR

For `BareAttrReader` to read bare attributes, `ir::Input` and `ir::Statement` need raw `syn::Attribute` lists:

```rust
// In ir::Input<L>
pub raw_attrs: Vec<syn::Attribute>,  // NEW — populated from syn::DeriveInput.attrs

// In ir::Statement<L>
pub raw_attrs: Vec<syn::Attribute>,  // NEW — populated from syn::Variant.attrs (enum)
                                     //        or syn::DeriveInput.attrs (struct)
```

This is populated during `FromDeriveInput` / `FromVariant` parsing. Purely additive, no existing code breaks.

## Migration Path

### Phase 1: Add raw attrs to IR (non-breaking)

1. Add `raw_attrs: Vec<syn::Attribute>` to `ir::Input<L>` and `ir::Statement<L>`.
2. Populate during `FromDeriveInput` / `FromVariant` parsing.
3. No existing behavior changes.

### Phase 2: Introduce `PropertyValueReader` + constructors (non-breaking)

1. Add `pub trait PropertyValueReader` to `generators/property/context.rs`.
2. Implement `PropertyValueReader for PropertyKind` (extract existing match logic + validation).
3. Add `pub struct BareAttrReader` implementing `PropertyValueReader`.
4. Change `DeriveProperty.kind: PropertyKind` to `DeriveProperty.reader: Box<dyn PropertyValueReader>`.
5. Add `DeriveProperty::bare_attr()` constructor (easy tier).
6. Add `DeriveProperty::with_reader()` constructor (middle tier).
7. Keep `DeriveProperty::new(kind: PropertyKind, ...)` with same signature (backward compatible).
8. Re-export `PropertyValueReader` and `BareAttrReader` from `generators/property/mod.rs`.
9. Update scan.rs: `self.kind.global_value(input)` -> `self.reader.global_value(input)`, validation via `self.reader.validate(input)`.
10. Update statement.rs: `derive.kind.statement_value(statement)` -> `derive.reader.statement_value(statement)`.

### Phase 3: Update kirin-derive (non-breaking)

1. No change needed in `kirin-derive/src/lib.rs` — `new_property()` already calls `DeriveProperty::new(config.kind, ...)` which still works.

### Phase 4: Delete kirin-derive-dialect

1. Remove `crates/kirin-derive-dialect/` entirely (not in workspace, dead code).

## How Layout Extension and Property Extension Relate

They remain **separate**, serving different purposes:

- **Layout extension** (`Layout` trait): For complex derives that need custom attribute schemas and full Scan/Emit pipelines. Example: `EvalCallLayout` for `CallSemantics`.

- **Property extension** (`DeriveProperty::bare_attr()` or `::with_reader()`): For bool properties. Easy tier needs just a string attribute name; middle tier allows custom `PropertyValueReader` logic while reusing emit machinery.

- **Complex derives** (HasParser, PrettyPrint): Fully independent pipelines. Neither mechanism applies.

The **unification point** is `raw_attrs` on IR types — `BareAttrReader` reads from it, and Layout-based derives could also use it if needed.

## Example: Simple property — `#[derive(IsQuantum)]`

### Proc-macro implementation (~10 lines)

```rust
// In my-quantum-dialect-derive/src/lib.rs

extern crate proc_macro;
use proc_macro::TokenStream;
use syn::parse_macro_input;
use kirin_derive_core::generators::property::DeriveProperty;

#[proc_macro_derive(IsQuantum, attributes(kirin, wraps, quantum))]
pub fn derive_is_quantum(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let mut derive = DeriveProperty::bare_attr(
        "quantum",                     // bare attribute name
        "::my_quantum_dialect",        // default crate path
        "IsQuantum",                   // trait name
        "is_quantum",                  // trait method
        "bool",                        // return type
    );
    match derive.emit(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}
```

### Usage

```rust
use my_quantum_dialect::{IsQuantum, Dialect};

#[derive(Dialect, IsQuantum)]
#[kirin(type = MyType)]
#[quantum]  // marks entire dialect as quantum
pub enum QuantumOps {
    Hadamard(SSAValue, ResultValue),
    #[quantum]  // per-variant override
    CNOT(SSAValue, SSAValue, ResultValue),
}
```

### Generated code

```rust
impl IsQuantum for QuantumOps {
    fn is_quantum(&self) -> bool {
        match self {
            Self::Hadamard(..) => true || false,   // global=true, variant=false
            Self::CNOT(..) => true || true,         // global=true, variant=true
        }
    }
}
```

The downstream author writes zero trait impls, zero Layout types, zero Scan/Emit code. They only need `DeriveProperty::bare_attr()` and their `#[proc_macro_derive]` function.

## Example: Middle tier — custom reader with validation

A downstream crate wants `#[derive(IsIdempotent)]` where idempotent implies pure (cross-property validation):

```rust
// In my-dialect-derive/src/lib.rs

use kirin_derive_core::generators::property::{DeriveProperty, PropertyValueReader, BareAttrReader};
use kirin_derive_core::ir;
use kirin_derive_core::prelude::StandardLayout;

struct IdempotentReader(BareAttrReader);

impl PropertyValueReader for IdempotentReader {
    fn global_value(&self, input: &ir::Input<StandardLayout>) -> bool {
        self.0.global_value(input)
    }

    fn statement_value(&self, statement: &ir::Statement<StandardLayout>) -> bool {
        self.0.statement_value(statement)
    }

    fn validate(&self, input: &ir::Input<StandardLayout>) -> darling::Result<()> {
        // Custom validation: idempotent requires pure
        if self.0.global_value(input) && !input.attrs.pure {
            return Err(darling::Error::custom(
                "#[idempotent] requires #[kirin(pure)]"
            ));
        }
        Ok(())
    }
}

#[proc_macro_derive(IsIdempotent, attributes(kirin, wraps, idempotent))]
pub fn derive_is_idempotent(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let reader = IdempotentReader(BareAttrReader::new("idempotent"));
    let mut derive = DeriveProperty::with_reader(
        reader,
        "::my_dialect",
        "IsIdempotent",
        "is_idempotent",
        "bool",
    );
    match derive.emit(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}
```

The middle tier reuses all of `DeriveProperty`'s scan/emit machinery — the author only implements the value reading and validation logic.

## Example: Complex derive — HasParser

Complex derives are out of scope for this design. They already work via independent pipelines:

- `HasParser` / `PrettyPrint`: `kirin-chumsky-format` pipeline with `#[chumsky(...)]` attributes
- `CallSemantics` / `Interpretable`: `kirin-derive-interpreter` with `EvalCallLayout`

These do not use `DeriveProperty` and are unaffected by this change.

## Impact on Existing Code

### Files that change

| File | Change |
|------|--------|
| `crates/kirin-derive-core/src/ir/` (Input struct) | Add `raw_attrs: Vec<syn::Attribute>`, populate in `FromDeriveInput` |
| `crates/kirin-derive-core/src/ir/` (Statement struct) | Add `raw_attrs: Vec<syn::Attribute>`, populate in `FromVariant`/`FromDeriveInput` |
| `crates/kirin-derive-core/src/generators/property/context.rs` | Add `pub trait PropertyValueReader`, `pub struct BareAttrReader`. Change `kind` field to `reader: Box<dyn PropertyValueReader>`. Add `bare_attr()` and `with_reader()` constructors. Impl `PropertyValueReader for PropertyKind`. |
| `crates/kirin-derive-core/src/generators/property/scan.rs` | `self.kind.global_value()` -> `self.reader.global_value()`. Move validation to `self.reader.validate()`. `self.kind.statement_value()` -> `self.reader.statement_value()`. |
| `crates/kirin-derive-core/src/generators/property/statement.rs` | `derive.kind.statement_value()` -> `derive.reader.statement_value()` |
| `crates/kirin-derive-core/src/generators/property/mod.rs` | Re-export `PropertyValueReader`, `BareAttrReader` alongside existing `DeriveProperty` |
| `crates/kirin-derive-dialect/` | **Delete entirely** |

### Files that do NOT change

- `crates/kirin-derive/src/lib.rs` — `DeriveProperty::new(kind, ...)` signature unchanged
- `crates/kirin-derive-core/src/ir/attrs.rs` — Hardcoded bool fields stay for built-in properties
- `crates/kirin-derive-core/src/generators/property/emit.rs` — Generic over value source, no change
- `crates/kirin-derive-core/src/ir/layout.rs` — Unchanged
- `crates/kirin-derive-interpreter/` — Independent pipeline
- `crates/kirin-chumsky-derive/` — Independent pipeline
- `crates/kirin-chumsky-format/` — Independent pipeline

### Test impact

- Existing snapshot tests pass unchanged (same generated code).
- New tests for `BareAttrReader` with a custom attribute.
- `kirin-derive-dialect` tests deleted with the crate.

## Resolved Questions

1. **`PropertyValueReader` visibility**: `pub`. Downstream developers can implement it for custom logic (middle tier).

2. **`BareAttrReader` visibility**: `pub`. Usable directly with `with_reader()` for composition (e.g., wrapping in a validating reader).

3. **`BareAttrReader` vs `#[kirin(quantum)]`**: Only bare `#[quantum]` is supported (matching `#[callable]` and `#[wraps]` precedent). Darling's `#[kirin(...)]` namespace is for built-in properties only.

4. **`Box<dyn>` vs generic**: `Box<dyn PropertyValueReader>` is fine — `DeriveProperty` is created once per derive invocation, no hot path.

5. **Keep `PropertyKind` enum?**: Yes. It groups the 4 built-in properties and their cross-property validation rules. It implements `PropertyValueReader`.
