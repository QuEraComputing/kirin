# Result Conventions And Nested Execution

## Structural Value Lists Use Raw `Product<V>`

The public interpreter protocol should use raw `kirin_ir::Product<V>` directly
for structural lists of runtime values such as:

- block arguments
- call arguments
- fork target arguments

No extra wrapper type is introduced for these structural lists.

## `Return` And `Yield` Stay Single-Valued

The semantic execution protocol uses `Return(V)` and `Yield(V)`, not
`Return(Product<V>)` or `Yield(Product<V>)`.

This preserves the intended rule that multiple outward results are a dialect
convention, often expressible as sugar over one product-valued semantic result,
rather than a second framework-level result transport mechanism.

## No Global `ProductValue` Requirement

The core interpreter must not impose a global runtime-value trait for packing
or unpacking product values.

If a dialect wants an implicit multi-result convention, such as a value enum
variant like `Tuple(Product<Self>)`, the dialect author handles that logic in
the relevant `Interpretable` and `ConsumeResult` implementations.

The framework owns execution mechanics. The dialect owns value-convention
policy.

## Generic, Dialect-Owned Result Consumption

Nested execution boundaries are handled through a generic consumer trait:

```rust
trait ConsumeResult<'ir, I: Interpreter<'ir>> {
    fn consume_result<L>(
        &self,
        interp: &mut I,
        value: I::Value,
    ) -> Result<(), I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir;
}
```

This trait is intentionally generic rather than call-specific or yield-specific.
It should work for any statement that starts nested execution and later needs to
map one semantic result value back into outward-facing results.

Examples include:

- function call statements
- `scf.if`
- `scf.for`
- future compound graph-node operations

In v1, `ConsumeResult` should be implemented only by statement definitions.

## `Call` Stays In The Effect Algebra

`Call` must remain in `ExecEffect`. Making call execution a synchronous Rust
function returning `V` would move recursion to Rust call frames and would weaken
stepping, debugging, and explicit control over nested execution.

Instead, the runtime handles `ExecEffect::Call` by:

1. storing the pending consumer statement and resume cursor
2. pushing a new callee frame
3. running the callee on the interpreter-managed frame stack
4. receiving `Return(V)`
5. popping the callee frame
6. restoring the caller
7. invoking `ConsumeResult` on the pending consumer statement

This keeps recursion and nested execution explicit and debugger-friendly.

## Dialect Examples

### Single-Result Call Convention

A dialect can define a call op that expects exactly one outward result and
write the returned value directly in `ConsumeResult`.

No packing or unpacking support is needed.

### Multi-Result Call Convention

A dialect can define a call op with multiple outward result slots and implement
its own unpacking logic in `ConsumeResult`.

For example, a dialect-specific value enum may contain a
`Tuple(Product<Self>)` variant. The dialect then unpacks that variant in
`ConsumeResult` and writes each outward result slot itself.

If the returned value does not match the dialect's convention, the dialect
raises an error there.

### `Return(SSAValue)` Or `Yield(SSAValue)`

If a dialect chooses a single SSA operand form, its `Interpretable`
implementation simply reads that value and emits `ExecEffect::Return(v)` or
`ExecEffect::Yield(v)`.

Any outward arity adaptation is handled by the consuming statement, not by the
framework.

### `Return(Vec<SSAValue>)` Or `Yield(Vec<SSAValue>)`

If a dialect chooses an explicit multi-operand form, its `Interpretable`
implementation is responsible for packing those values into one semantic `V`
before emitting `Return(v)` or `Yield(v)`.

That packing policy is also dialect-owned.

## Error Ownership

Framework errors cover execution-mechanics failures, for example:

- invalid cursor transitions
- missing stage or frame
- invalid block-argument arity at a control transfer boundary
- unexpected `Return` or `Yield` at the wrong runtime boundary

Dialect errors cover result-convention failures, for example:

- a callsite cannot unpack a returned value into its outward result slots
- a return or yield op cannot pack multiple SSA operands into one semantic
  value
- a graph boundary consumer rejects the nested result value shape

This keeps ownership coherent: the framework handles control mechanics; the
dialect handles value meaning.
