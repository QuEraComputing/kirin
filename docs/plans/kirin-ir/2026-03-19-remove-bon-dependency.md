# Remove bon Dependency from kirin-ir

## Problem

`kirin-ir` depends on `bon = "3.8"` (Cargo.toml line 7) for builder generation on 5 impl blocks. `bon` is a heavy proc-macro dependency that pulls in `bon-macros`, `darling 0.23`, `prettyplease`, `rustversion`, and full `syn`. For a core IR crate that should have minimal dependencies, this is unnecessary overhead. The builders it generates are simple named-argument constructors that can be replaced with hand-written code.

## Research Findings

### All `#[bon::bon]` usage sites

1. **`src/builder/staged.rs:56-73`** -- `BuilderStageInfo<L>::ssa()` method
   ```rust
   #[bon::bon]
   impl<L: Dialect> BuilderStageInfo<L> {
       #[builder(finish_fn = new)]
       pub fn ssa(
           &mut self,
           #[builder(into)] name: Option<String>,
           ty: L::Type,
           kind: BuilderSSAKind,
       ) -> SSAValue { ... }
   }
   ```
   Generates: `self.ssa().name("foo").ty(ArithType::I32).kind(BuilderSSAKind::Test).new()`

2. **`src/node/block.rs:63-89`** -- `BlockInfo<L>::new()` constructor
   ```rust
   #[bon::bon]
   impl<L: Dialect> BlockInfo<L> {
       #[builder(finish_fn = new)]
       pub(crate) fn new(
           parent: Option<Region>,
           name: Option<Symbol>,
           node: LinkedListNode<Block>,
           arguments: Vec<BlockArgument>,
           statements: Option<LinkedList<Statement>>,
           terminator: Option<Statement>,
       ) -> Self { ... }
   }
   ```
   Generates: `BlockInfo::new().parent(p).name(n).node(nd).arguments(args).new()`

3. **`src/node/region.rs:21-38`** -- `RegionInfo<L>::new()` constructor
   ```rust
   #[bon::bon]
   impl<L: Dialect> RegionInfo<L> {
       #[builder(finish_fn = new)]
       pub fn new(
           id: Region,
           parent: Option<Statement>,
           blocks: LinkedList<Block>,
       ) -> Self { ... }
   }
   ```
   Generates: `RegionInfo::new().id(id).parent(stmt).blocks(list).new()`

4. **`src/node/function/specialized.rs:37-57`** -- `SpecializedFunctionInfo<L>::new()` constructor
   ```rust
   #[bon::bon]
   impl<L: Dialect> SpecializedFunctionInfo<L> {
       #[builder(finish_fn = new)]
       pub fn new(
           id: SpecializedFunction,
           signature: Signature<L::Type>,
           body: Statement,
           backedges: Option<Vec<SpecializedFunction>>,
       ) -> Self { ... }
   }
   ```

5. **`src/pipeline.rs:159-186`** -- `Pipeline<S>::add_stage()` method
   ```rust
   #[bon::bon]
   impl<S> Pipeline<S> {
       #[builder(finish_fn = new)]
       pub fn add_stage(&mut self, mut stage: S, #[builder(into)] name: Option<String>) -> CompileStage
       where S: StageMeta { ... }
   }
   ```
   Generates: `pipeline.add_stage().stage(ctx).name("llvm_ir").new()`

### bon features actually used

- `#[builder(finish_fn = new)]` -- renames the terminal method from `build()` to `new()`
- `#[builder(into)]` -- adds `.into()` conversion on the parameter (used on `name: Option<String>`)
- Optional parameters via `Option<T>` -- bon makes these skippable in the builder chain

### Caller analysis

Need to find all call sites using the builder pattern to understand the API surface.

The `ssa()` builder is used extensively in tests and dialect builders. The `add_stage()` builder is used in pipeline construction. The `BlockInfo::new()`, `RegionInfo::new()`, and `SpecializedFunctionInfo::new()` builders are used internally in kirin-ir's own builder infrastructure.

### Does removing bon also remove darling 0.20?

No. `cargo tree` shows only darling 0.23 in the entire workspace. bon-macros v3.9.1 uses darling 0.23 (same as the workspace). Removing bon removes bon-macros, prettyplease, and rustversion -- but not a separate darling version.

### Dependency reduction from removing bon

Removing bon eliminates:
- `bon` (runtime, though it's mostly re-exports)
- `bon-macros` (proc-macro: darling 0.23, ident_case, prettyplease, proc-macro2, quote, rustversion, syn)
- `prettyplease` (a code formatter, pulled only by bon-macros)
- `rustversion` (proc-macro for version detection)

The darling/syn/quote/proc-macro2 dependencies remain (used by kirin-derive-ir). The net savings are `bon`, `bon-macros`, `prettyplease`, and `rustversion`.

## Proposed Changes

### Strategy: Replace with plain methods that take all parameters directly

For constructors (sites 2-4), replace the builder with a plain `fn new(...)` that takes all required fields. Optional fields use `Option<T>` parameters directly.

For the `ssa()` method (site 1) and `add_stage()` method (site 5), the builder pattern provides genuine ergonomic value (optional `name` parameter, `into` conversion). Replace with a small hand-written builder struct or method overloads.

### Site 1: `BuilderStageInfo::ssa()` -> hand-written builder

Replace `#[bon::bon]` with a small `SSABuilder` struct:

```rust
pub struct SSABuilder<'a, L: Dialect> {
    stage: &'a mut BuilderStageInfo<L>,
    name: Option<String>,
    ty: L::Type,
    kind: BuilderSSAKind,
}

impl<L: Dialect> BuilderStageInfo<L> {
    pub fn ssa(&mut self, ty: L::Type, kind: BuilderSSAKind) -> SSABuilder<'_, L> {
        SSABuilder { stage: self, name: None, ty, kind }
    }
}

impl<L: Dialect> SSABuilder<'_, L> {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn new(self) -> SSAValue {
        // move existing body here
    }
}
```

Alternative (simpler): make `name` an `Option<impl Into<String>>` parameter on a plain method. Callers pass `None::<String>` or `Some("name")`. This loses some ergonomics but is simpler.

### Site 2: `BlockInfo::new()` -> plain constructor

```rust
impl<L: Dialect> BlockInfo<L> {
    pub(crate) fn new(
        parent: Option<Region>,
        name: Option<Symbol>,
        node: LinkedListNode<Block>,
        arguments: Vec<BlockArgument>,
        statements: Option<LinkedList<Statement>>,
        terminator: Option<Statement>,
    ) -> Self {
        Self {
            parent, name, node, arguments,
            statements: statements.unwrap_or_default(),
            terminator,
            _marker: std::marker::PhantomData,
        }
    }
}
```

This is `pub(crate)` so only internal callers need updating. Find all `BlockInfo::new().` call chains and convert to direct calls.

### Site 3: `RegionInfo::new()` -> plain constructor

```rust
impl<L: Dialect> RegionInfo<L> {
    pub fn new(id: Region, parent: Option<Statement>, blocks: LinkedList<Block>) -> Self {
        Self { id, parent, blocks, _marker: std::marker::PhantomData }
    }
}
```

### Site 4: `SpecializedFunctionInfo::new()` -> plain constructor

```rust
impl<L: Dialect> SpecializedFunctionInfo<L> {
    pub fn new(
        id: SpecializedFunction,
        signature: Signature<L::Type>,
        body: Statement,
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> Self {
        Self {
            id, signature, body,
            backedges: backedges.unwrap_or_default(),
            invalidated: false,
        }
    }
}
```

### Site 5: `Pipeline::add_stage()` -> method with optional name

```rust
impl<S> Pipeline<S> {
    pub fn add_stage(&mut self, mut stage: S, name: Option<impl Into<String>>) -> CompileStage
    where S: StageMeta
    {
        let id = CompileStage::new(Id(self.stages.len()));
        stage.set_stage_id(Some(id));
        if let Some(n) = name {
            let sym = self.global_symbols.intern(n.into());
            stage.set_stage_name(Some(sym));
        }
        self.stages.push(stage);
        id
    }
}
```

Callers change from `pipeline.add_stage().stage(ctx).name("x").new()` to `pipeline.add_stage(ctx, Some("x"))` or `pipeline.add_stage(ctx, None::<&str>)`.

### Cargo.toml change

Remove line 7:
```diff
-bon = "3.8"
```

## Migration Steps

1. **Find all call sites** for each builder. Use grep for `.ssa().`, `BlockInfo::new().`, `RegionInfo::new().`, `SpecializedFunctionInfo::new().`, `.add_stage().`.
2. **Replace site 2** (`BlockInfo::new`) first -- it's `pub(crate)`, lowest risk.
3. **Replace site 3** (`RegionInfo::new`) -- public but simple.
4. **Replace site 4** (`SpecializedFunctionInfo::new`) -- public but simple.
5. **Replace site 5** (`Pipeline::add_stage`) -- public API, needs careful caller migration. This is referenced in doc comments (`src/pipeline.rs:47`) and likely in tests and examples.
6. **Replace site 1** (`BuilderStageInfo::ssa`) -- most-used builder, decide between hand-written builder struct or plain method. The builder pattern is used extensively in tests (`stage.ssa().name("a").ty(ArithType::I32).kind(BuilderSSAKind::Test).new()`).
7. **Remove `bon = "3.8"`** from Cargo.toml.
8. **Update doc comments** that reference the builder API (e.g., `src/pipeline.rs:47`).
9. **Run full test suite**.

## Risk Assessment

- **API breakage**: `ssa()` and `add_stage()` are public APIs used by dialect crates, tests, and examples. The builder-to-plain-method change modifies every call site. This is the highest-risk part.
- **`ssa()` builder ergonomics**: The current builder lets callers omit `name` entirely (it defaults to `None`). A plain method with `Option<impl Into<String>>` requires explicit `None::<String>`. A hand-written builder preserves the ergonomics but adds code.
- **Doc comments**: Several doc comments show builder usage examples. Must update all.
- **`add_stage()` in toy-lang and tests**: Used in pipeline construction throughout the codebase. Grep for all `.add_stage()` calls.

## Testing Strategy

- `cargo build -p kirin-ir` -- core crate compiles without bon
- `cargo nextest run -p kirin-ir` -- unit tests pass
- `cargo build --workspace` -- all downstream crates compile
- `cargo nextest run --workspace` -- all tests pass
- `cargo test --doc --workspace` -- doc tests pass
- `cargo tree -p kirin-ir --depth 1` -- verify bon is gone
- `cargo tree -p kirin-ir | grep -c prettyplease` -- verify prettyplease is gone
