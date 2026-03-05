# kirin-derive-toolkit Rustdoc Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add comprehensive rustdoc to `kirin-derive-toolkit` so downstream derive macro developers can understand the architecture and API without reading all 54 source files.

**Architecture:** Top-down narrative — crate-level overview in `lib.rs` establishes the pipeline model, module docs explain each layer's role, and key types get API examples. No standalone markdown guide; everything lives in Rust doc comments.

**Tech Stack:** Rust doc comments (`///`, `//!`), `cargo doc`, `cargo test --doc`

---

### Task 1: Crate-Level Documentation (`lib.rs`)

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/lib.rs`

**Step 1: Add crate-level doc comment**

Add `//!` doc comment block at the top of `lib.rs` (before `pub mod` declarations). Content:

```rust
//! A metaprogramming toolkit for building Kirin dialect derive macros.
//!
//! # Architecture
//!
//! The toolkit follows a four-stage pipeline:
//!
//! ```text
//! syn::DeriveInput ──► Input<L> ──► Scan ──► Emit ──► TokenStream
//!      (Rust AST)     (IR parse)  (collect)  (codegen)  (output)
//! ```
//!
//! ## Layers
//!
//! | Layer | Modules | Purpose |
//! |-------|---------|---------|
//! | **IR** | [`ir`], [`ir::fields`] | Parsed representation of derive input — types, fields, attributes |
//! | **Visitors** | [`scan`], [`emit`] | Two-pass visitor pattern: scan collects metadata, emit generates code |
//! | **Generators** | [`generators`] | Pre-built generators for common derives (builder, field iterators, properties) |
//! | **Tokens** | [`tokens`], [`codegen`] | Typed code-block builders (`TraitImpl`, `MatchExpr`, etc.) and utilities |
//! | **Support** | [`context`], [`derive`], [`stage`], [`misc`] | Pre-computed state, metadata extraction, stage parsing |
//!
//! ## Quick Start
//!
//! Most derives follow this pattern:
//!
//! 1. Parse: `Input::<StandardLayout>::from_derive_input(&ast)?`
//! 2. Implement [`Scan`] to collect per-statement metadata
//! 3. Implement [`Emit`] to generate code for each statement
//! 4. Or compose pre-built [`generators`] via `input.generate().with(gen).emit()?`
//!
//! ## Layout Extensibility
//!
//! [`StandardLayout`] works for most derives. If your derive needs custom attributes
//! on statements or fields (e.g., `#[callable]`), define a custom [`Layout`] impl.
//! See [`ir::Layout`] for details.
//!
//! [`Scan`]: scan::Scan
//! [`Emit`]: emit::Emit
//! [`Layout`]: ir::Layout
//! [`StandardLayout`]: ir::StandardLayout
```

**Step 2: Verify docs build**

Run: `cargo doc -p kirin-derive-toolkit --no-deps 2>&1 | head -20`
Expected: No errors, warnings OK.

**Step 3: Commit**

```
docs(derive-toolkit): add crate-level architecture overview
```

---

### Task 2: IR Module Documentation

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/ir/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/layout.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/input.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/statement/definition.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/statement/accessors.rs`

**Step 1: Add module doc to `ir/mod.rs`**

```rust
//! Parsed IR representation of derive macro input.
//!
//! This module provides a three-level hierarchy that mirrors how Kirin dialect
//! types are structured:
//!
//! ```text
//! Input<L>          ── top-level struct/enum being derived
//!   └─ Statement<L> ── each variant (enum) or the single body (struct)
//!       └─ FieldInfo<L> ── each field, classified by category
//! ```
//!
//! The [`Layout`] trait parameterizes the IR so different derives can attach
//! custom attributes at each level. [`StandardLayout`] uses `()` for all extras
//! and is the right choice for most derives.
//!
//! # Parsing
//!
//! ```ignore
//! let ir = Input::<StandardLayout>::from_derive_input(&ast)?;
//! ```
```

**Step 2: Add doc comments to `Layout` trait in `layout.rs`**

```rust
/// Extension point for per-derive custom attributes.
///
/// Each associated type corresponds to a level in the IR hierarchy where
/// a derive macro can inject extra parsed attributes. [`StandardLayout`]
/// sets all extras to `()` — use it unless your derive needs custom
/// attributes like `#[callable]` or `#[format(...)]`.
///
/// # Custom Layout Example
///
/// ```ignore
/// struct MyLayout;
///
/// impl Layout for MyLayout {
///     type StatementExtra = MyStatementAttrs;  // parsed from variant attrs
///     type ExtraGlobalAttrs = ();
///     type ExtraStatementAttrs = ();
///     type ExtraFieldAttrs = ();
/// }
/// ```
```

Add doc to `StandardLayout`:
```rust
/// Default layout with no custom attributes at any level.
///
/// Use this for derives that only need the built-in `#[kirin(...)]` attributes.
```

**Step 3: Add doc comments to `Input`, `Data`, `DataStruct`, `DataEnum`, `VariantRef` in `input.rs`**

`Input<L>`:
```rust
/// Top-level parsed representation of a derive macro input.
///
/// Wraps a `syn::DeriveInput` with Kirin-specific attribute parsing and
/// field classification. Access the parsed statements via [`data`](Self::data).
///
/// # Parsing
///
/// ```ignore
/// let input = Input::<StandardLayout>::from_derive_input(&ast)?;
/// match &input.data {
///     Data::Struct(s) => { /* single statement */ }
///     Data::Enum(e) => { /* multiple variants */ }
/// }
/// ```
```

`Data<L>`:
```rust
/// The body of the derive input — either a single struct or an enum with variants.
```

`DataStruct<L>`:
```rust
/// A struct-style input, containing a single [`Statement`].
///
/// Derefs to the inner `Statement<L>` for convenience.
```

`DataEnum<L>`:
```rust
/// An enum-style input, containing one [`Statement`] per variant.
///
/// Use [`iter_variants`](Self::iter_variants) for iteration that distinguishes
/// wrapper variants (marked with `#[wraps]`) from regular ones.
```

`VariantRef`:
```rust
/// Reference to an enum variant, distinguishing wrappers from regular variants.
///
/// Wrapper variants delegate to an inner type via `#[wraps]`; regular variants
/// have their own fields.
```

**Step 4: Add doc comments to `Statement` in `definition.rs`**

```rust
/// A single IR operation — either a struct body or one enum variant.
///
/// Each statement has a name, parsed `#[kirin(...)]` options, and a list of
/// classified [`FieldInfo`] entries. If the variant uses `#[wraps]`, the
/// [`wraps`](Self::wraps) field contains the delegation target.
///
/// # Field Access
///
/// ```ignore
/// for field in stmt.arguments() {
///     // SSAValue fields
/// }
/// for field in stmt.results() {
///     // ResultValue fields
/// }
/// for field in stmt.values() {
///     // Plain Rust-type fields
/// }
/// ```
```

**Step 5: Add doc comments to accessors in `accessors.rs`**

Add one-line `///` doc to each public method:

- `iter_all_fields` → `/// Iterates all fields regardless of category.`
- `arguments` → `/// Iterates fields classified as [`FieldCategory::Argument`] (SSAValue types).`
- `results` → `/// Iterates fields classified as [`FieldCategory::Result`] (ResultValue types).`
- `blocks` → `/// Iterates fields classified as [`FieldCategory::Block`].`
- `successors` → `/// Iterates fields classified as [`FieldCategory::Successor`].`
- `regions` → `/// Iterates fields classified as [`FieldCategory::Region`].`
- `values` → `/// Iterates fields classified as [`FieldCategory::Value`] (plain Rust types).`
- `field_count` → `/// Returns the total number of fields.`
- `named_field_idents` → `/// Returns identifiers for all named fields (empty for tuple structs).`
- `is_tuple_style` → `/// Returns `true` if fields are positional (tuple struct/variant).`
- `field_name_to_index` → `/// Maps field names to their positional indices.`
- `field_bindings` → `/// Builds [`FieldBindings`](crate::codegen::FieldBindings) with the given variable prefix.`
- `collect_fields` → `/// Clones all fields into a new `Vec`.`

**Step 6: Verify and commit**

Run: `cargo doc -p kirin-derive-toolkit --no-deps 2>&1 | head -20`
Expected: No errors.

```
docs(derive-toolkit): add IR module and type documentation
```

---

### Task 3: Fields Module Documentation

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/ir/fields/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/fields/collection.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/fields/wrapper.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/fields/index.rs`

**Step 1: Add module doc to `fields/mod.rs`**

```rust
//! Field classification algebra for IR statements.
//!
//! Every field in a Kirin statement is automatically classified by its Rust type:
//!
//! | Rust Type | Category | Meaning |
//! |-----------|----------|---------|
//! | `SSAValue` / `SSAValue<T>` | [`Argument`](FieldCategory::Argument) | SSA input value |
//! | `ResultValue` / `ResultValue<T>` | [`Result`](FieldCategory::Result) | SSA output value |
//! | `Block` | [`Block`](FieldCategory::Block) | Basic block reference |
//! | `Successor` | [`Successor`](FieldCategory::Successor) | Control-flow successor |
//! | `Region` / `Region<T>` | [`Region`](FieldCategory::Region) | Nested region |
//! | `Symbol` | [`Symbol`](FieldCategory::Symbol) | Symbol reference |
//! | anything else | [`Value`](FieldCategory::Value) | Plain Rust value |
//!
//! Each field also tracks its [`Collection`] wrapping: `Single`, `Vec`, or `Option`.
```

**Step 2: Add doc comments to types in `fields/mod.rs`**

`FieldCategory`:
```rust
/// Classification of a field's semantic role in an IR statement.
///
/// Determined automatically from the field's Rust type during parsing.
```

`FieldData<L>`:
```rust
/// Semantic data associated with a field, varying by [`FieldCategory`].
///
/// `Argument` and `Result` variants carry an `ssa_type` expression.
/// `Value` carries the original Rust type and optional default/into metadata.
```

`FieldInfo<L>`:
```rust
/// Complete metadata about a single field in a [`Statement`](super::Statement).
///
/// Combines positional info (`index`, `ident`), collection wrapping, and
/// category-specific data. Use [`category()`](Self::category) to branch on
/// the field's role.
///
/// ```ignore
/// match field.category() {
///     FieldCategory::Argument => { /* field.ssa_type() is Some */ }
///     FieldCategory::Value => { /* field.value_type() is Some */ }
///     _ => {}
/// }
/// ```
```

**Step 3: Add doc comments in `collection.rs`**

`Collection`:
```rust
/// How a field's base type is wrapped in a collection.
///
/// A field typed `Vec<SSAValue>` is classified as `Argument` with
/// `Collection::Vec`. The collection affects generated parser and
/// constructor code.
```

`from_type`: `/// Detects collection wrapping from a `syn::Type`.`
`wrap_type`: `/// Wraps a base type token in the collection (e.g., `Vec<base>`).`
`wrap_parser`: `/// Wraps a parser expression for this collection kind.`

**Step 4: Add doc comments in `wrapper.rs`**

`Wrapper`:
```rust
/// Metadata for a `#[wraps]` delegation field.
///
/// When an enum variant has `#[wraps]`, it delegates to an inner type
/// (usually another dialect's statement). The wrapper tracks which field
/// holds the inner type and its `syn::Type`.
```

**Step 5: Add doc comments in `index.rs`**

`FieldIndex`:
```rust
/// Positional identity of a field — either named (`foo`) or positional (`0`).
```

`FieldName`:
```rust
/// Display-ready field reference: named fields emit their ident, positional
/// fields emit their index.
```

**Step 6: Verify and commit**

Run: `cargo doc -p kirin-derive-toolkit --no-deps 2>&1 | head -20`

```
docs(derive-toolkit): add fields module documentation
```

---

### Task 4: Scan and Emit Visitor Documentation

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/scan.rs`
- Modify: `crates/kirin-derive-toolkit/src/emit.rs`

**Step 1: Add module doc and trait doc to `scan.rs`**

Module doc:
```rust
//! Input traversal visitor for collecting metadata from IR.
//!
//! [`Scan`] walks the IR hierarchy (`Input` → `Statement` → fields) and lets
//! you override hooks at each level. All methods have default implementations
//! that recurse into children, so you only override what you need.
//!
//! The free functions (e.g., [`scan_input`], [`scan_statement`]) provide the
//! default traversal logic — call them from your overrides to continue walking.
//!
//! # Example: Collecting Statement Names
//!
//! ```ignore
//! struct NameCollector {
//!     names: Vec<String>,
//! }
//!
//! impl<'ir> Scan<'ir, StandardLayout> for NameCollector {
//!     fn scan_statement(
//!         &mut self,
//!         stmt: &'ir Statement<StandardLayout>,
//!     ) -> darling::Result<()> {
//!         self.names.push(stmt.name.to_string());
//!         scan::scan_statement(self, stmt) // continue into fields
//!     }
//! }
//! ```
```

Trait doc for `Scan`:
```rust
/// Visitor trait for traversing IR and collecting metadata.
///
/// Override specific methods to intercept nodes of interest. Call the
/// corresponding free function (e.g., [`scan_statement`]) from your
/// override to continue the default traversal into children.
///
/// All 13 methods have default implementations that delegate to
/// the free functions in this module.
```

**Step 2: Add module doc and trait doc to `emit.rs`**

Module doc:
```rust
//! Code generation visitor for producing `TokenStream` output from IR.
//!
//! [`Emit`] mirrors [`Scan`](crate::scan::Scan) but returns `TokenStream`
//! instead of `()`. Override hooks at each level to generate code; the
//! default implementations concatenate children's output.
//!
//! Typically used after a [`Scan`](crate::scan::Scan) pass that collected
//! the metadata needed for code generation.
//!
//! # Example: Generating Match Arms
//!
//! ```ignore
//! impl<'ir> Emit<'ir, StandardLayout> for MyEmitter {
//!     fn emit_statement(
//!         &mut self,
//!         stmt: &'ir Statement<StandardLayout>,
//!     ) -> darling::Result<TokenStream> {
//!         let name = &stmt.name;
//!         Ok(quote! { Self::#name { .. } => todo!() })
//!     }
//! }
//! ```
```

Trait doc for `Emit`:
```rust
/// Visitor trait for generating `TokenStream` output from IR.
///
/// Override specific methods to emit code for nodes of interest.
/// The default implementations concatenate children's output.
/// Call the corresponding free function (e.g., [`emit_statement`])
/// from your override to include children's output in yours.
```

**Step 3: Verify and commit**

Run: `cargo doc -p kirin-derive-toolkit --no-deps 2>&1 | head -20`

```
docs(derive-toolkit): add Scan and Emit visitor documentation
```

---

### Task 5: Generator Framework and Pre-Built Generators Documentation

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/generator.rs`
- Modify: `crates/kirin-derive-toolkit/src/generators/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/generators/builder/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/generators/field/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/generators/property/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/generators/marker.rs`
- Modify: `crates/kirin-derive-toolkit/src/generators/stage_info.rs`

**Step 1: Add docs to `generator.rs`**

Module doc:
```rust
//! Composable code generator framework.
//!
//! The [`Generator`] trait and [`GenerateBuilder`] let you compose multiple
//! code generators and run them over an [`Input`](crate::ir::Input) in one pass.
//!
//! ```ignore
//! let tokens = input.generate()
//!     .with(DeriveBuilder::new(ir_path))
//!     .with(DeriveProperty::new(PropertyKind::Pure, "IsPure", "is_pure"))
//!     .emit()?;
//! ```
```

`Generator` trait:
```rust
/// A composable code generator that produces `TokenStream` from a [`DeriveContext`].
///
/// Implement this trait for custom generators, or use closures
/// (blanket impl provided for `Fn(&DeriveContext<L>) -> Result<Vec<TokenStream>>`).
```

`GenerateBuilder`:
```rust
/// Fluent builder for composing and executing [`Generator`]s.
///
/// Created via [`Input::generate()`](crate::ir::Input::generate). Chain
/// generators with [`.with()`](Self::with), then call [`.emit()`](Self::emit)
/// to run them all and concatenate output.
```

**Step 2: Add module doc to `generators/mod.rs`**

```rust
//! Pre-built generators for common Kirin derive patterns.
//!
//! | Generator | Emits |
//! |-----------|-------|
//! | [`builder::DeriveBuilder`] | Constructor `new()` functions |
//! | [`field::DeriveFieldIter`] | Field iterator trait impls (`HasArguments`, `HasResults`, etc.) |
//! | [`property::DeriveProperty`] | Property trait impls (`IsTerminator`, `IsPure`, etc.) |
//! | [`marker::derive_marker`] | Marker trait `Type` associated type |
//! | [`stage_info::generate`] | `StageMeta` and `HasStageInfo` impls for stage enums |
//!
//! These can be used standalone or composed via [`GenerateBuilder`](crate::generator::GenerateBuilder).
```

**Step 3: Add one-line doc to each sub-module's public type**

`generators/builder/mod.rs` — add to `DeriveBuilder`:
```rust
/// Generates constructor functions for IR statements.
///
/// Emits `new(...)` methods on structs, or per-variant constructors for enums,
/// based on the statement's fields and `#[kirin(builder = ...)]` options.
```

`generators/field/mod.rs` — add to `DeriveFieldIter`:
```rust
/// Generates field iterator trait implementations.
///
/// Produces `HasArguments`, `HasResults`, `HasBlocks`, `HasSuccessors`, and
/// `HasRegions` impls with both immutable and mutable iterators.
```

Add to `FieldIterKind`:
```rust
/// Which field category to generate iterators for.
```

`generators/property/mod.rs` — add to `DeriveProperty`:
```rust
/// Generates boolean property trait implementations.
///
/// Reads `#[kirin(terminator)]`, `#[kirin(constant)]`, `#[kirin(pure)]`,
/// and `#[kirin(speculatable)]` attributes to emit trait impls that
/// return `true` or `false` per variant.
```

Add to `PropertyKind`:
```rust
/// Which property trait to generate.
```

`generators/marker.rs` — add to `derive_marker`:
```rust
/// Generates a marker trait impl with a `Type` associated type alias.
///
/// Used to stamp the IR type identity onto dialect types.
```

`generators/stage_info.rs` — add to `generate`:
```rust
/// Generates [`StageMeta`] and [`HasStageInfo`] impls for a stage enum.
///
/// Parses `#[stage(name = "...", StageInfo<Dialect>)]` attributes on each
/// variant and emits the stage dispatch, name/ID accessors, and dialect
/// resolution methods.
```

**Step 4: Verify and commit**

Run: `cargo doc -p kirin-derive-toolkit --no-deps 2>&1 | head -20`

```
docs(derive-toolkit): add generator framework and pre-built generator docs
```

---

### Task 6: Tokens Module Documentation

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/tokens/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/tokens/trait_impl.rs`
- Modify: `crates/kirin-derive-toolkit/src/tokens/match_expr.rs`
- Modify: `crates/kirin-derive-toolkit/src/tokens/pattern.rs`

**Step 1: Add module doc to `tokens/mod.rs`**

```rust
//! Typed code-block builders for generating Rust syntax.
//!
//! Instead of assembling `TokenStream` with raw `quote!` calls, these builders
//! provide structured, composable code generation with compile-time shape
//! guarantees.
//!
//! | Builder | Generates |
//! |---------|-----------|
//! | [`TraitImpl`] | `impl Trait for Type { ... }` blocks |
//! | [`InherentImpl`] | `impl Type { ... }` blocks |
//! | [`MatchExpr`] | `match subject { arm => body, ... }` expressions |
//! | [`Pattern`] | Destructuring patterns (`Foo { a, b }` or `Foo(a, b)`) |
//! | [`StructDef`], [`EnumDef`] | Type definitions |
//! | [`DelegationCall`] | Forwarding calls through `#[wraps]` fields |
//!
//! All builders implement `ToTokens` so they can be interpolated directly
//! in `quote!` expressions.
```

**Step 2: Add doc comments to key types in `trait_impl.rs`**

`TraitImpl`:
```rust
/// Builder for `impl Trait for Type { ... }` blocks.
///
/// ```ignore
/// let imp = TraitImpl::new(generics, quote!(MyTrait), quote!(MyType))
///     .trait_generics(quote!(<'ir, L>))
///     .method(Method {
///         name: format_ident!("my_method"),
///         self_arg: quote!(&self),
///         params: vec![quote!(x: u32)],
///         return_type: Some(quote!(bool)),
///         body: quote! { x > 0 },
///     })
///     .assoc_type(format_ident!("Output"), quote!(u32));
/// // imp implements ToTokens
/// ```
```

`Method`:
```rust
/// A method definition inside a trait impl.
```

`AssocType`:
```rust
/// An associated type definition (`type Name = Ty;`).
```

`AssocConst`:
```rust
/// An associated constant definition (`const NAME: Ty = val;`).
```

**Step 3: Add doc comments in `match_expr.rs`**

`MatchExpr`:
```rust
/// Builder for `match` expressions.
///
/// ```ignore
/// let m = MatchExpr {
///     subject: quote!(self),
///     arms: vec![
///         MatchArm {
///             pattern: quote!(Self::Add { .. }),
///             guard: None,
///             body: quote! { true },
///         },
///     ],
/// };
/// // m implements ToTokens → `match self { Self::Add { .. } => { true } }`
/// ```
```

`MatchArm`:
```rust
/// A single arm in a [`MatchExpr`].
```

**Step 4: Add doc comments in `pattern.rs`**

`Pattern`:
```rust
/// Destructuring pattern for struct/enum fields.
///
/// Renders as `{ a, b, c }` for named fields or `(a, b, c)` for tuple fields.
/// Built automatically by [`Statement::field_bindings`](crate::ir::Statement::field_bindings).
```

**Step 5: Verify and commit**

Run: `cargo doc -p kirin-derive-toolkit --no-deps 2>&1 | head -20`

```
docs(derive-toolkit): add tokens module documentation
```

---

### Task 7: Codegen Utilities and Support Module Documentation

**Files:**
- Modify: `crates/kirin-derive-toolkit/src/codegen/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/codegen/constructor.rs`
- Modify: `crates/kirin-derive-toolkit/src/codegen/generics_builder.rs`
- Modify: `crates/kirin-derive-toolkit/src/codegen/field_bindings.rs`
- Modify: `crates/kirin-derive-toolkit/src/context.rs`
- Modify: `crates/kirin-derive-toolkit/src/derive/mod.rs`
- Modify: `crates/kirin-derive-toolkit/src/derive/input.rs`
- Modify: `crates/kirin-derive-toolkit/src/stage.rs`
- Modify: `crates/kirin-derive-toolkit/src/misc.rs`
- Modify: `crates/kirin-derive-toolkit/src/ir/attrs.rs`

**Step 1: Add module doc to `codegen/mod.rs`**

```rust
//! Utilities for generating constructor expressions, managing generics, and
//! binding field variables in generated code.
```

**Step 2: Add docs to codegen types**

`ConstructorBuilder`:
```rust
/// Builds constructor expressions for structs or enum variants.
///
/// Handles both named (`Foo { a, b }`) and tuple (`Foo(a, b)`) styles,
/// mapping each field through a user-provided closure.
///
/// ```ignore
/// let ctor = ConstructorBuilder::new_struct(&type_name, is_tuple);
/// let tokens = ctor.build(&stmt.fields, |field| {
///     let name = field.name_ident(Span::call_site());
///     quote!(#name)
/// });
/// ```
```

`GenericsBuilder`:
```rust
/// Manipulates generics for generated trait impls.
///
/// Adds the `'ir` lifetime and optional `L: Language` type parameter
/// required by Kirin trait impls.
```

`FieldBindings`:
```rust
/// Captures field variable names for use in destructuring patterns and
/// generated code bodies.
///
/// For named fields, preserves original names with an optional prefix.
/// For tuple fields, generates `{prefix}0`, `{prefix}1`, etc.
```

**Step 3: Add docs to `context.rs`**

`DeriveContext`:
```rust
/// Pre-computed context shared across generators during code emission.
///
/// Built once from an [`Input`](crate::ir::Input) and passed to each
/// [`Generator`](crate::generator::Generator). Contains pre-built patterns,
/// wrapper detection, and per-statement contexts to avoid repeated scanning.
```

`StatementContext`:
```rust
/// Pre-computed context for a single statement/variant.
///
/// Includes the destructuring [`Pattern`](crate::tokens::Pattern) and
/// wrapper status, ready for use in match arms.
```

**Step 4: Add docs to `derive/mod.rs` and `derive/input.rs`**

Module doc for `derive/mod.rs`:
```rust
//! Metadata extraction and path construction helpers for derive macros.
```

`InputMeta`:
```rust
/// Extracted metadata from an [`Input`](crate::ir::Input): name, generics,
/// crate path, IR type, and whether it's an enum.
///
/// Use [`path_builder`](Self::path_builder) to construct fully-qualified
/// paths for generated trait references.
```

`PathBuilder`:
```rust
/// Constructs fully-qualified paths relative to the user's crate configuration.
///
/// Respects `#[kirin(crate = ...)]` overrides, falling back to the provided
/// default crate path.
```

**Step 5: Add docs to `stage.rs`**

Module doc:
```rust
//! Stage enum parsing utilities for `StageMeta` derives.
//!
//! Parses `#[stage(...)]` attributes on enum variants to extract stage names
//! and dialect type parameters.
```

`StageVariantInfo`:
```rust
/// Parsed metadata from a single stage enum variant.
```

One-line docs on public functions:
- `parse_ir_crate_path` → `/// Extracts the `#[stage(crate = "...")]` override from attributes.`
- `parse_stage_variant` → `/// Parses a single enum variant's `#[stage(...)]` attributes.`
- `parse_stage_variants` → `/// Parses all variants of a stage enum.`

**Step 6: Add docs to `misc.rs`**

Module doc:
```rust
//! Miscellaneous utilities: case conversion, type inspection, attribute parsing.
```

One-line docs on each function:
- `strip_path` → `/// Extracts the last segment of a path as an `Ident`.`
- `from_str` → `/// Parses a string into any `syn::Parse` type.`
- `to_camel_case` → `/// Converts a string to CamelCase.`
- `to_snake_case` → `/// Converts a string to snake_case.`
- `is_type` → `/// Checks if a type's last path segment matches the given name.`
- `is_vec_type` → `/// Checks if a type is `Vec<T>` where `T` matches the given name.`
- `is_type_in_generic` → `/// Checks if a type appears as a generic argument of another type.`
- `is_type_in` → `/// Checks if a type matches with a custom segment predicate.`
- `parse_attribute` → `/// Parses a named attribute's nested meta items.`
- `error_unknown_attribute` → `/// Creates a "unknown attribute" error for use in attribute parsers.`

**Step 7: Add docs to `ir/attrs.rs`**

One-line docs on each public type:
- `GlobalOptions` → `/// Normalized global options from `#[kirin(...)]` on the input type.`
- `StatementOptions` → `/// Per-variant/statement options from `#[kirin(...)]`.`
- `KirinFieldOptions` → `/// Field-level options from `#[kirin(...)]` on fields.`
- `BuilderOptions` → `/// Builder function configuration: enabled (default name) or named.`
- `DefaultValue` → `/// Default value for a field: `Default::default()` or a custom expression.`
- `KirinStructOptions` → `/// Raw darling-parsed options for struct inputs.`
- `KirinEnumOptions` → `/// Raw darling-parsed options for enum inputs.`

**Step 8: Verify and commit**

Run: `cargo doc -p kirin-derive-toolkit --no-deps 2>&1 | head -20`

```
docs(derive-toolkit): add codegen, context, and support module documentation
```

---

### Task 8: Final Verification

**Files:** None (read-only verification)

**Step 1: Run full doc build**

Run: `cargo doc -p kirin-derive-toolkit --no-deps 2>&1`
Expected: No errors. Warnings about unused imports are OK.

**Step 2: Run doc tests (if any examples use `ignore` vs `no_run`)**

Run: `cargo test --doc -p kirin-derive-toolkit 2>&1`
Expected: Passes (all examples use `ignore` so no doc tests actually run).

**Step 3: Spot-check rendered docs**

Run: `cargo doc -p kirin-derive-toolkit --no-deps --open`
Manually verify:
- Crate-level doc shows architecture table
- Module list shows descriptions
- Key types (`Input`, `Scan`, `Emit`, `TraitImpl`) have doc + examples

**Step 4: Commit message if any fixes needed**

```
docs(derive-toolkit): fix doc build issues
```
