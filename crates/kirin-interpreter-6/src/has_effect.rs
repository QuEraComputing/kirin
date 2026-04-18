/// Declares the dialect-specific effect type contributed to a composed language.
///
/// Only implement this for dialects with effects that CANNOT be expressed as
/// `Core<V, C>`. Dialects like `Arith` and `SCF` — whose ops always reduce to
/// `Core` variants — do NOT implement `HasEffect`. Their ops return `Core` directly.
///
/// The GAT `type Effect<V>` is `V`-parameterized only, keeping the effect type
/// free of interpreter-type references.
///
/// # When to implement
/// A quantum backend dialect with gate application effects:
/// ```rust,ignore
/// impl HasEffect for QuantumDialect {
///     type Effect<V> = QuantumEffect<V>;
/// }
/// ```
///
/// # Derive support
/// `#[derive(ComposedEffect)]` reads `HasEffect` for each variant of a language
/// enum and generates the composed `LangEffect<V, C>` coproduct with all
/// `Lift` / `Project` impls. (derive not yet implemented — compose manually.)
pub trait HasEffect {
    type Effect<V>;
}
