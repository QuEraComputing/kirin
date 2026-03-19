# Add Feature Flags to `kirin` Workspace Package

## Problem

All 7 dialect crates depend on `kirin.workspace = true`, which unconditionally pulls in `kirin-chumsky`, `kirin-prettyless`, `kirin-lexer`, and `chumsky`. Interpreter-only or analysis-only users compile the full parser and printer stack.

**Key design decision (user):** Dialects should continue depending on the top-level `kirin` package — it is the user-facing facade. The fix is adding features to `kirin` itself, not decoupling dialects to depend on internal crates.

## Proposed Design

### Features on the `kirin` crate

```toml
# kirin/Cargo.toml
[features]
default = ["parser", "pretty", "derive"]
parser = ["kirin-chumsky"]
pretty = ["kirin-prettyless"]
derive = ["kirin-derive"]
interpret = ["kirin-interpreter"]
```

### How dialects use it

```toml
# kirin-arith/Cargo.toml
[dependencies]
kirin = { workspace = true }  # gets parser + pretty + derive by default

[features]
default = []
interpret = ["kirin/interpret", "kirin-interpreter"]
```

For interpreter-only consumers downstream:
```toml
kirin-arith = { version = "...", default-features = false, features = ["interpret"] }
```

### Conditional compilation in `kirin` lib.rs

```rust
pub use kirin_ir as ir;

#[cfg(feature = "parser")]
pub use kirin_chumsky as parsers;

#[cfg(feature = "pretty")]
pub use kirin_prettyless as pretty;

pub mod prelude {
    pub use kirin_ir::*;
    #[cfg(feature = "parser")]
    pub use kirin_chumsky::prelude::*;
    #[cfg(feature = "pretty")]
    pub use kirin_prettyless::prelude::*;
}
```

### Dialect source changes

Dialects keep `use kirin::prelude::*` — it just brings in fewer items when features are disabled. The `HasParser` and `PrettyPrint` derives need conditional compilation:

```rust
#[derive(Dialect)]
#[cfg_attr(feature = "parser", derive(HasParser, PrettyPrint))]
pub enum Arith<T: CompileTimeValue> { ... }
```

Where `feature = "parser"` here refers to the dialect's own feature that activates `kirin/parser`.

## Implementation Steps

1. Add `parser`, `pretty`, `derive` features to `kirin/Cargo.toml` with existing deps made optional.
2. Gate `kirin/src/lib.rs` re-exports behind features.
3. Gate prelude re-exports behind features.
4. Update dialect crates to conditionally derive `HasParser`/`PrettyPrint`.
5. Verify `cargo build -p kirin --no-default-features` compiles (IR-only).
6. Verify `cargo build --workspace` (all features) still works.

## Interaction with Other Plans

- **Plan 14 (feature-gate prettyless in chumsky)**: Still relevant — `kirin-chumsky` itself can feature-gate `kirin-prettyless` for internal decoupling.
- **Plan 16 (remove bon)**: Independent, reduces `kirin-ir` deps regardless of feature flags.

## Risk Assessment

**Medium risk.** Feature gating `kirin` re-exports is straightforward, but conditional derives on dialect enums (`cfg_attr`) add visual noise. The benefit is that downstream consumers can compile subsets of the stack.

**Backward compatible:** `default = ["parser", "pretty", "derive"]` ensures existing users see no change.

## Testing Strategy

- `cargo build -p kirin --no-default-features` — IR-only compiles
- `cargo build -p kirin --features parser` — parser without pretty
- `cargo build --workspace` — everything works as before
- All 1045 tests pass
