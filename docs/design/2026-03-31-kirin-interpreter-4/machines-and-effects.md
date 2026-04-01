# Machines And Effects

## Machine Trait

Every stateful component is a Machine:

```rust
trait Machine {
    type Effect;
    type Error;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<(), Self::Error>;
}
```

The machine receives effects and mutates its internal state to handle them.
There is no `Shell<Stop>` return — effect handling is fully internal to the
machine. If the effect requires cursor or frame changes, the machine (i.e. the
interpreter) performs those changes directly.

This is simpler than interpreter-3's `ConsumeEffect → Shell<Stop>` chain. The
interpreter is itself a Machine, and its `consume_effect` is where cursor and
frame mutations happen.

## Machine Composition

Composition follows the structural rules from interpreter-3:

- dialect composition is sum-like (enum)
- machine composition is product-like (struct)
- effect composition is sum-like (enum)
- error composition is sum-like (enum)

```rust
// Dialect sum
#[wraps]
enum MyLanguage {
    Arith(Arith),
    Cf(Cf),
    Func(Func),
}

// Machine product
struct MyMachine {
    arith: ArithMachine,
    cf: CfMachine,
    func: FuncMachine,
}

// Effect sum
enum MyEffect {
    Arith(ArithEffect),
    Cf(CfEffect),
    Func(FuncEffect),
}

// Error sum
enum MyError {
    Arith(ArithError),
    Cf(CfError),
    Func(FuncError),
}
```

The composite machine's `consume_effect` dispatches to the appropriate
sub-machine:

```rust
impl Machine for MyMachine {
    type Effect = MyEffect;
    type Error = MyError;

    fn consume_effect(&mut self, effect: MyEffect) -> Result<(), MyError> {
        match effect {
            MyEffect::Arith(e) => self.arith.consume_effect(e).map_err(MyError::Arith),
            MyEffect::Cf(e) => self.cf.consume_effect(e).map_err(MyError::Cf),
            MyEffect::Func(e) => self.func.consume_effect(e).map_err(MyError::Func),
        }
    }
}
```

## Mixed-Flavor Effect System

The central insight: dialect authors want mutation when they can handle
everything locally, and return effects when the concern spreads beyond the
statement.

This is achieved by the `Interpretable` trait signature:

```rust
trait Interpretable<I: Interpreter> {
    type Effect;
    type Error;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Self::Effect, Self::Error>;
}
```

The `&mut I` parameter gives full mutation access: read/write values, project
machine state, call execution seeds. The return type gives an effect channel
for deferred control flow.

### When to mutate

- reading and writing SSA values
- updating dialect-local machine state (counters, caches)
- executing inline bodies via execution seeds (`exec_block`, `invoke`)

### When to return effects

- changing the cursor position (jump to successor block)
- requesting a function call (push new frame)
- returning from a function (pop frame)
- forking for abstract interpretation
- stopping execution for a semantic reason

### Examples

Arithmetic — pure mutation, no effect:
```rust
impl<I: Interpreter> Interpretable<I> for Add {
    type Effect = (); // no effect needed
    type Error = I::Error;

    fn interpret(&self, interp: &mut I) -> Result<(), I::Error> {
        let lhs = interp.read(self.lhs)?;
        let rhs = interp.read(self.rhs)?;
        interp.write(self.result, lhs + rhs)?;
        Ok(())
    }
}
```

Control flow — effect for cursor change:
```rust
impl<I: Interpreter> Interpretable<I> for Branch {
    type Effect = CfEffect<I::Value>;
    type Error = I::Error;

    fn interpret(&self, interp: &mut I) -> Result<CfEffect<I::Value>, I::Error> {
        let args = interp.read_many(&self.args)?;
        Ok(CfEffect::Jump(self.target, args))
    }
}
```

Structured control flow — inline execution, no deferred effect:
```rust
impl<I: Interpreter> Interpretable<I> for If {
    type Effect = ();
    type Error = I::Error;

    fn interpret(&self, interp: &mut I) -> Result<(), I::Error> {
        let cond = interp.read(self.condition)?;
        let result = if cond.is_truthy() {
            interp.exec_block(self.then_block, &[])?
        } else {
            interp.exec_block(self.else_block, &[])?
        };
        interp.write(self.result, result)?;
        Ok(())
    }
}
```

Function call — effect for frame management:
```rust
impl<I: Interpreter> Interpretable<I> for Call {
    type Effect = FuncEffect<I::Value>;
    type Error = I::Error;

    fn interpret(&self, interp: &mut I) -> Result<FuncEffect<I::Value>, I::Error> {
        let args = interp.read_many(&self.args)?;
        let callee = interp.resolve_callee(self.target, &args)?;
        Ok(FuncEffect::Call {
            callee,
            args,
            results: self.results.clone(),
        })
    }
}
```

Function call — alternative with inline invocation:
```rust
impl<I: Interpreter> Interpretable<I> for Call {
    type Effect = ();
    type Error = I::Error;

    fn interpret(&self, interp: &mut I) -> Result<(), I::Error> {
        let args = interp.read_many(&self.args)?;
        let callee = interp.resolve_callee(self.target, &args)?;
        let result = interp.invoke(callee, &args)?;
        interp.write_results(&self.results, result)?;
        Ok(())
    }
}
```

## Effect Handling

The `()` effect type has special semantics: it means "advance to next
statement". The interpreter treats `Ok(())` as a cursor advance.

For non-unit effects, the interpreter lifts the dialect-local effect into its
top-level effect type and calls `self.consume_effect(lifted_effect)`. The
interpreter's own `consume_effect` dispatches:

- cursor effects (Jump) → update cursor position
- frame effects (Call, Return) → push/pop frames
- dialect machine effects → delegate to inner machine's `consume_effect`
- stop effects → latch stop and halt execution

The interpreter knows how to classify effects because it owns the top-level
effect type and the inner machine composition.

## Effect Classification Via Marker Traits

The interpreter does not depend on a specific effect enum. Instead, effects are
classified via marker traits:

```rust
trait IsAdvance {
    fn is_advance(&self) -> bool;
}

trait IsJump<V> {
    fn as_jump(&self) -> Option<(Block, &[V])>;
}

trait IsCall<V> {
    fn as_call(&self) -> Option<CallInfo<V>>;
}

trait IsReturn<V> {
    fn as_return(&self) -> Option<&V>;
}

trait IsYield<V> {
    fn as_yield(&self) -> Option<&V>;
}

trait IsStop<V> {
    fn as_stop(&self) -> Option<&V>;
}

trait IsFork<V> {
    fn as_fork(&self) -> Option<&[(Block, Vec<V>)]>;
}
```

The interpreter's `consume_effect` uses these traits to classify effects
without knowing the concrete enum type. This allows downstream developers to
compose their own effect enums and implement the marker traits.

The framework provides a base `CursorEffect<V>` enum that dialect authors can
reuse:

```rust
enum CursorEffect<V> {
    Advance,
    Jump(Block, Vec<V>),
    Fork(Vec<(Block, Vec<V>)>),
}
```

But this is a convenience, not a requirement. Dialect authors can define their
own effect types as long as they implement the relevant marker traits.

## Unit Effect Convention

When `Interpretable::Effect = ()`, it means the dialect produced NO effect.
This is distinct from "advance" — it means the statement had nothing to say
about control flow.

The interpreter must still decide what to do when it receives no effect. The
default behavior is to advance the cursor, but this is an interpreter policy
decision, not an effect semantic.

Simple dialects (arith, constant, bitwise, cmp) return `Ok(())` — no effect.
The interpreter advances because no effect was produced.

## Lift And Project

All type conversions are unified under Lift/Project:

```rust
// Infallible embedding
trait Lift<From> {
    fn lift(from: From) -> Self;
}

// Infallible projection (reference)
trait Project<To: ?Sized> {
    fn project(&self) -> &To;
}

trait ProjectMut<To: ?Sized> {
    fn project_mut(&mut self) -> &mut To;
}

// Fallible variants
trait TryLift<From>: Sized {
    type Error;
    fn try_lift(from: From) -> Result<Self, Self::Error>;
}

trait TryProject<To>: Sized {
    type Error;
    fn try_project(self) -> Result<To, Self::Error>;
}

// Convenience reversal
trait LiftInto<Target>: Sized {
    fn lift_into(self) -> Target;
}

trait TryLiftInto<Target>: Sized {
    type Error;
    fn try_lift_into(self) -> Result<Target, Self::Error>;
}
```

Identity impls for all types. Blanket upgrades from infallible to fallible.

### Applied to each domain

**Machines** (product → component via Project):
```rust
impl Project<ArithMachine> for MyMachine {
    fn project(&self) -> &ArithMachine { &self.arith }
}
impl ProjectMut<ArithMachine> for MyMachine {
    fn project_mut(&mut self) -> &mut ArithMachine { &mut self.arith }
}
```

**Effects** (component → sum via Lift):
```rust
impl Lift<CfEffect> for MyEffect {
    fn lift(from: CfEffect) -> Self { MyEffect::Cf(from) }
}
```

**Errors** (component → sum via Lift):
```rust
impl Lift<CfError> for MyError {
    fn lift(from: CfError) -> Self { MyError::Cf(from) }
}
```

The interpreter provides forwarding helpers so dialect authors write:
```rust
let machine: &mut ArithMachine = interp.project_machine_mut();
```

rather than navigating the projection chain themselves.

## Dialect Machine Examples

### Stateless machine

Many dialects have no per-dialect state. Their machine is `()`:

```rust
impl Machine for () {
    type Effect = ();
    type Error = Infallible;

    fn consume_effect(&mut self, _: ()) -> Result<(), Infallible> {
        Ok(())
    }
}
```

### Cursor-directive machine

Control flow dialects produce cursor effects but own no state:

```rust
struct CfMachine;

enum CfEffect<V> {
    Jump(Block, Vec<V>),
    Fork(Vec<(Block, Vec<V>)>), // abstract interpretation
}

impl Machine for CfMachine {
    type Effect = CfEffect;
    type Error = CfError;

    fn consume_effect(&mut self, _effect: CfEffect) -> Result<(), CfError> {
        // CfMachine has no state to update.
        // The interpreter handles the cursor change.
        Ok(())
    }
}
```

Wait — this creates a problem. If `CfMachine::consume_effect` is a no-op, who
actually updates the cursor? The interpreter does, because it sees the
`MyEffect::Cf(CfEffect::Jump(...))` at the top level and knows to update its
cursor.

The layered consume_effect flow is:

1. `Interpretable::interpret` returns a local `CfEffect::Jump(block, args)`
2. The interpreter lifts this to `MyEffect::Cf(CfEffect::Jump(block, args))`
3. The interpreter's `consume_effect` matches on `MyEffect::Cf(...)` and
   updates its cursor to jump to `block` with `args`

The inner dialect machine's `consume_effect` is called only when the dialect
machine actually needs to update its own state. For cursor-only effects, the
interpreter handles everything.

This means the interpreter's `consume_effect` needs to understand cursor-level
effect semantics. The interpreter does NOT blindly delegate all sub-effects to
inner machines.

### Resolution

The interpreter's effect handling is a two-phase dispatch:

1. **Interpreter-level handling**: check if the effect is a cursor/frame effect
   that the interpreter handles directly (Jump, Call, Return, Stop)
2. **Machine delegation**: for effects the interpreter doesn't directly handle,
   delegate to the inner machine's `consume_effect`

This is implemented via a trait or method on the effect type:

```rust
trait EffectKind<V> {
    fn as_cursor_directive(&self) -> Option<CursorDirective<V>>;
}
```

Or more simply, via the `Lift` trait — the interpreter knows its own top-level
effect type and can pattern-match directly.

## Deferred: Abstract Interpretation

The mixed-flavor system naturally extends to abstract interpretation:

- `exec_block` on an abstract interpreter runs the block to fixpoint
- `exec_region` on an abstract interpreter runs the CFG to fixpoint
- `CfEffect::Fork(targets)` signals undecidable branches
- The abstract interpreter's `consume_effect` propagates all branch targets

The key invariant: dialect authors write `Interpretable` once. The concrete and
abstract interpreters provide different implementations of the execution seed
methods.
