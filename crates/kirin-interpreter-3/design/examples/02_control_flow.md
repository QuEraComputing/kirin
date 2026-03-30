# Example 2: Control Flow Dialect (Branch)

Control flow dialects use base effects for jumps and returns. They also demonstrate
returning interpreter errors (e.g., unsupported nondeterministic branches).

This is the pattern used by `kirin-cf`.

## Key Characteristics

- Still `type Effect = Infallible` and `type Error = Infallible` — no machine-specific types
- Uses `Effect::Jump` instead of `BindValue + Advance`
- Shows error return: `Err(InterpreterError::unsupported(...).into())`

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
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, Infallible>, InterpError<Infallible>>
    {
        let cond = interp.read(self.condition)?;
        let (block, arg_ssas) = match cond.is_truthy() {
            Some(true) => (self.true_block, &self.true_args),
            Some(false) => (self.false_block, &self.false_args),
            None => return Err(InterpreterError::unsupported("nondeterministic branch").into()),
        };
        let args: SmallVec<[_; 2]> = arg_ssas.iter()
            .map(|a| interp.read(*a))
            .collect::<Result<_, _>>()?;
        Ok(Effect::Jump(block, args))
    }
}
```

## Notes

- `BranchCondition::is_truthy()` returns `Option<bool>` — `None` means undecidable (abstract
  interpretation would handle this via a Fork machine effect; concrete interpretation errors).
- `InterpreterError::unsupported(...)` is a base interpreter error. `.into()` converts to
  `InterpError<Infallible>` via `From<InterpreterError>`, then `Err(...)` returns it.
- Jump arguments are read from SSA values and packed into a `SmallVec` for the block args.
