# Example 2: Control Flow Dialect (Branch)

Control flow dialects use base effects for jumps and returns. They also demonstrate
returning interpreter errors as effects (e.g., unsupported nondeterministic branches).

This is the pattern used by `kirin-cf`.

## Key Characteristics

- Still `type Effect = ()` and `type Error = Infallible` — no machine-specific types
- Uses `BaseEffect::Jump` instead of `BindValue + Advance`
- Shows error-as-effect pattern: `InterpreterError::unsupported(...).try_lift()`

## Code

```rust
struct Branch<T> {
    condition: SSAValue,
    true_block: Block,
    false_block: Block,
    true_args: Vec<SSAValue>,
    false_args: Vec<SSAValue>,
    _phantom: PhantomData<T>,
}

impl<I: Interpreter> Interpretable<I> for Branch<T>
where
    I::Value: BranchCondition,
{
    type Effect = ();
    type Error = Infallible;

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<()>, I::Error<Infallible>> {
        let cond = interp.read(self.condition)?;
        let (block, arg_ssas) = match cond.is_truthy() {
            Some(true) => (self.true_block, &self.true_args),
            Some(false) => (self.false_block, &self.false_args),
            None => return InterpreterError::unsupported("nondeterministic branch").try_lift(),
        };
        let args: SmallVec<[_; 2]> = arg_ssas.iter()
            .map(|a| interp.read(*a))
            .collect::<Result<_, _>>()?;
        BaseEffect::Jump(block, args).try_lift()
    }
}
```

## Notes

- `BranchCondition::is_truthy()` returns `Option<bool>` — `None` means undecidable (abstract
  interpretation would use Fork; concrete interpretation errors).
- `InterpreterError::unsupported(...)` is a base interpreter error. `.try_lift()` converts it
  to `I::Error<Infallible>` via the Interpreter slot, then returns it as an `Err`.
- Jump arguments are read from SSA values and packed into a `SmallVec` for the block args.
