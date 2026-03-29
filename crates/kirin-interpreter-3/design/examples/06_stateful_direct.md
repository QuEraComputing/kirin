# Example 6: Stateful Dialect (Direct Machine Mutation)

A dialect with its own machine state, mutated directly via `ProjectMut<M>`. This is the
simple path for concrete interpreters — no machine effects needed.

## Key Characteristics

- The dialect defines a machine struct (`MemoryMachine`)
- `I: Interpreter + ProjectMut<MemoryMachine>` — additional per-dialect bound
- Sequential borrows: read values first, then project machine — no borrow conflicts
- Still returns `BaseEffect` — the mutation happens in-place, not through effects

## Code

```rust
struct MemoryMachine {
    storage: HashMap<u64, Vec<u8>>,
}

struct Store<T> {
    addr: SSAValue,
    value: SSAValue,
    _phantom: PhantomData<T>,
}

impl<I: Interpreter + ProjectMut<MemoryMachine>> Interpretable<I> for Store<T>
where
    I::Value: Into<u64> + AsRef<[u8]>,
{
    type Effect = ();
    type Error = Infallible;

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<()>, I::Error<Infallible>> {
        // Read values first (borrows interp immutably)
        let addr: u64 = interp.read(self.addr)?.into();
        let value = interp.read(self.value)?;

        // Then project machine (borrows interp mutably — sequential, no conflict)
        let mem: &mut MemoryMachine = interp.project_mut();
        mem.storage.insert(addr, value.as_ref().to_vec());

        BaseEffect::Advance.try_lift()
    }
}
```

## Borrow Pattern

The sequential borrow pattern avoids Rust's aliasing restrictions:

```
interp.read(...)    ← immutable borrow (released before next line)
interp.read(...)    ← immutable borrow (released before next line)
interp.project_mut() ← mutable borrow (no conflict — previous borrows released)
```

This works because Rust allows sequential (non-overlapping) borrows. The dialect reads
all values it needs, then takes the mutable reference to its machine.

## When to Use Direct Mutation vs Machine Effects

- **Direct mutation** (this example): for concrete interpreters where the mutation is
  straightforward and doesn't need interception.
- **Machine effects** ([Example 7](07_stateful_effects.md)): when the mutation needs to go
  through the effect pipeline (e.g., abstract interpretation needs to process state changes
  differently, or effects need ordering guarantees relative to other effects).
