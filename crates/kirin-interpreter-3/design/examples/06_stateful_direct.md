# Example 6: Rejected Pattern (Direct Machine Mutation)

This example is intentionally negative. It shows the pattern that `kirin-interpreter-3`
rejects for dialect semantics.

The code sketch mentions `ProjectMut` only to illustrate the old escape hatch. It is not part
of the proposed public dialect-facing API.

## Rejected Pattern

```rust
impl<I> Interpretable<I> for Store<T>
where
    I: Interpreter + ProjectMut<MemoryMachine>,
    I::Value: Into<u64> + AsRef<[u8]>,
{
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Effect<I::Value, Infallible>, InterpError<Infallible>> {
        let addr: u64 = interp.read(self.addr)?.into();
        let value = interp.read(self.value)?;

        let mem: &mut MemoryMachine = interp.project_mut();
        mem.storage.insert(addr, value.as_ref().to_vec());

        Ok(Effect::Advance)
    }
}
```

## Why This Is Rejected

1. The write to `mem.storage` is semantically visible but not represented in the effect algebra.
2. The write cannot participate in `Seq` ordering.
3. The write cannot be lifted when the dialect is wrapped into a larger language.
4. An abstract or replaying interpreter cannot observe or reinterpret the change.

## Required Replacement

Stateful dialects must emit `Effect::Machine(de)` instead:

```rust
enum MemoryEffect {
    Store { addr: u64, bytes: Vec<u8> },
}

impl<I: Interpreter> Interpretable<I> for Store<T>
where
    I::Value: Into<u64> + AsRef<[u8]>,
{
    type Effect = MemoryEffect;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Effect<I::Value, MemoryEffect>, InterpError<Infallible>> {
        let addr: u64 = interp.read(self.addr)?.into();
        let value = interp.read(self.value)?;

        Ok(Effect::Machine(MemoryEffect::Store {
            addr,
            bytes: value.as_ref().to_vec(),
        }))
    }
}
```

That replacement keeps the state transition observable, ordered, and composable.
