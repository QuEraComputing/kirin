# U3: Derive Infrastructure (kirin-derive-toolkit + kirin-derive-ir) -- Code Quality Review

## Clippy / Lint Findings

### [P2] [confirmed] #[allow(clippy::large_enum_variant)] at ir/input.rs:115 (Data<L>)
Root cause: `Data::Struct(DataStruct<L>)` contains a full `Statement<L>` inline, while `Data::Enum(DataEnum<L>)` contains a `Vec<Statement<L>>` (heap-allocated). The struct variant is large because `Statement` carries fields, attrs, and name inline. Removable: possibly. Fix: Box the `DataStruct` variant (`Struct(Box<DataStruct<L>>)`). However, `Data` is typically constructed once and passed by reference, so the allocation cost is negligible. The current suppression is reasonable -- low priority.

### [P2] [confirmed] #[allow(clippy::large_enum_variant)] at ir/fields/data.rs:42 (FieldData<L>)
Root cause: `FieldData::Value` carries `syn::Type`, `Option<DefaultValue>`, `bool`, and `L::ExtraFieldAttrs`, which is significantly larger than the SSA-type-only variants. Removable: possibly. Fix: Box the `Value` variant. Same tradeoff as above -- these are constructed during derive parsing (once per compile) and rarely moved. Low priority.

## Duplication Findings

### [P2] [likely] FieldInfo accessor pairs -- ir/fields/info.rs
The file contains many small accessor methods that follow a repetitive pattern: `match &self.data { FieldData::X { field } => Some(field), _ => None }`. While each is individually small (2-5 lines), there are ~15 of them. Suggested abstraction: Could use a macro `field_accessor!(method_name, FieldData::Variant, field_name, ReturnType)`. Lines saved: ~30. However, the current explicit form is more readable and IDE-friendly, so this is low priority.

### [P3] [uncertain] field_iter_set.rs -- repetitive FieldIterTemplateSet generation
The 312-line file generates iterator structs, trait impls, and `Iterator` impls with significant structural repetition between struct and enum paths. This is inherent to the code generation pattern and not easily reduced without a meta-template layer.

## Rust Best Practices

### [P2] [likely] misc.rs is a grab-bag module (310 lines)
Contains `debug_dump`, `strip_path`, `from_str`, `to_camel_case`, `to_snake_case`, `is_type`, `is_type_in_generic`, and `find_wrapping_type`. These serve different purposes (debugging, string conversion, type inspection). Suggested split: `case_convert.rs` (camel/snake), `type_inspect.rs` (is_type, is_type_in_generic, find_wrapping_type), keep `debug_dump`/`from_str`/`strip_path` in a smaller `util.rs`.

### [P2] [likely] Manual Clone impl for FieldData<L> and FieldInfo<L>
Both `FieldData` and `FieldInfo` have manual `Clone` impls instead of `#[derive(Clone)]`. This is likely because `L::ExtraFieldAttrs` might not derive `Clone` uniformly. If `Layout::ExtraFieldAttrs: Clone` is already a bound, these could use `#[derive(Clone)]` and eliminate ~30 lines of boilerplate. Verify the trait bounds before changing.

## Strengths

- Template system (`TraitImplTemplate`, `MethodPattern`, `BuilderTemplate`) provides clean separation between code generation patterns and specific derive implementations.
- `DeriveContext` pre-computation of `StatementContext` (wrapper_type, wrapper_binding, pattern) avoids repeated inference in downstream codegen.
- `Layout` trait is a good extension point for derive-specific attributes without polluting core IR.
- `Input::from_derive_input` correctly filters `__`-prefixed variants for PhantomData carriers.
