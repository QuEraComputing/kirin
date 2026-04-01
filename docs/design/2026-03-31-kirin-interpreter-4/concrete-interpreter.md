# Concrete Interpreter

## Receipt Trait

A Receipt bundles all the type parameters needed for an interpreter
implementation. This avoids the large parameter lists that plagued previous
interpreter generics:

```rust
trait Receipt {
    type Language: Dialect;
    type Value: Clone;
    type Machine: Machine;
    type StageInfo: HasStageInfo<Self::Language>;
    type Error: From<InterpreterError>;
}
```

Concrete interpreters are parameterized by a single `R: Receipt`:

```rust
struct ConcreteInterpreter<'ir, R: Receipt> {
    pipeline: &'ir Pipeline<R::StageInfo>,
    frames: FrameStack<R::Value>,
    machine: R::Machine,
    cursor: Option<CursorState>,
    fuel: Option<u64>,
    // ...
}
```

Users define a receipt for their language:

```rust
struct MyReceipt;

impl Receipt for MyReceipt {
    type Language = MyLanguage;
    type Value = MyValue;
    type Machine = MyMachine;
    type StageInfo = StageInfo<MyLanguage>;
    type Error = MyError;
}
```

## Interpreter Trait Stack

The trait decomposition from kirin-interpreter v1 carries forward with
refinements:

```rust
/// SSA value read/write
trait ValueStore {
    type Value: Clone;
    type Error;

    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>;
    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error>;
    fn write_ssa(&mut self, ssa: SSAValue, value: Self::Value) -> Result<(), Self::Error>;
}

/// Pipeline and stage identity
trait PipelineAccess {
    type StageInfo;

    fn pipeline(&self) -> &Pipeline<Self::StageInfo>;
    fn current_stage(&self) -> CompileStage;
}

/// Complete interpreter = Machine + ValueStore + PipelineAccess
trait Interpreter: Machine + ValueStore + PipelineAccess {
    /// Access the top-level dialect machine
    fn machine(&self) -> &<Self as InterpreterMachine>::Machine;
    fn machine_mut(&mut self) -> &mut <Self as InterpreterMachine>::Machine;
}
```

The `Interpreter` trait is a blanket supertrait — anything implementing
`Machine + ValueStore + PipelineAccess + InterpreterMachine` gets `Interpreter`
automatically.

Actually, let's simplify. The interpreter needs to expose both its own Machine
impl (for top-level effect handling) and the inner dialect machine. We can do
this with:

```rust
trait Interpreter: Machine + ValueStore + PipelineAccess {
    type DialectMachine: Machine;

    fn dialect_machine(&self) -> &Self::DialectMachine;
    fn dialect_machine_mut(&mut self) -> &mut Self::DialectMachine;
}
```

But this creates confusion between the interpreter's own `Machine::Effect` and
the dialect machine's effect. Let's keep it simpler for the MVP:

The interpreter IS a Machine. Its `Effect` type is the top-level effect (sum of
dialect effects + framework effects). Dialect machine access is via
`Project`/`ProjectMut`:

```rust
trait Interpreter: Machine + ValueStore + PipelineAccess {
    // Provided method via Project
    fn project_machine<T: ?Sized>(&self) -> &T
    where
        Self: Project<T>;

    fn project_machine_mut<T: ?Sized>(&mut self) -> &mut T
    where
        Self: ProjectMut<T>;
}
```

## Frame and FrameStack

Carried forward from interpreter-4 draft and interpreter-2:

```rust
struct Frame<V, X = ()> {
    callee: SpecializedFunction,
    stage: CompileStage,
    values: FxHashMap<SSAValue, V>,
    extra: X,
}

struct FrameStack<V, X = ()> {
    frames: Vec<Frame<V, X>>,
    max_depth: Option<usize>,
}
```

For the concrete interpreter, `X` is an Activation struct that holds cursor
state:

```rust
struct Activation {
    cursor_stack: Vec<ExecutionCursor>,
    after_statement: Option<Statement>,
}
```

## Cursor Stack

The cursor stack is per-frame, owned by `Activation`. This decouples IR
traversal from the call stack:

- call = push frame (with new Activation containing initial cursor)
- return = pop frame (Activation is dropped)
- inline body execution = push/pop cursor within the SAME frame's Activation

```
Frame 2 (current): [BlockCursor(scf.for body)]
                    [RegionCursor(function entry)]
Frame 1 (caller):  [RegionCursor(function entry)]
Frame 0 (root):    [RegionCursor(main entry)]
```

When `exec_block` is called:
1. Push a new BlockCursor onto the current frame's cursor_stack
2. Run the block
3. Pop the BlockCursor
4. Return the result

When `invoke` is called:
1. Push a new Frame with a RegionCursor for the callee's entry
2. Run the callee
3. Pop the Frame
4. Return the result

## Concrete Interpreter Shape

```rust
struct ConcreteInterpreter<'ir, R: Receipt> {
    pipeline: &'ir Pipeline<R::StageInfo>,
    frames: FrameStack<R::Value, Activation>,
    machine: R::Machine,
    root_stage: CompileStage,
    fuel: Option<u64>,
    breakpoints: FxHashSet<Breakpoint>,
    last_stop: Option<R::Value>,
}
```

### Machine impl

The interpreter is a Machine. Its effect type is `R::Machine::Effect` — the
top-level dialect effect.

```rust
impl<'ir, R: Receipt> Machine for ConcreteInterpreter<'ir, R> {
    type Effect = <R::Machine as Machine>::Effect;
    type Error = R::Error;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<(), Self::Error> {
        // Pattern-match on the top-level effect to determine:
        // 1. Cursor-level effects → update cursor
        // 2. Frame-level effects → push/pop frames
        // 3. Dialect machine effects → delegate to self.machine
        self.machine.consume_effect(effect)
            .map_err(Into::into)
    }
}
```

Wait — this naive delegation doesn't work because cursor effects need to update
the interpreter's cursor, not the dialect machine. The interpreter needs to
intercept cursor-level effects.

### Effect Classification

The interpreter needs a way to classify effects. Two options:

**Option A: Trait-based classification**

```rust
trait ClassifyEffect<V> {
    fn classify(self) -> EffectAction<V, Self>
    where
        Self: Sized;
}

enum EffectAction<V, E> {
    /// Advance to next statement (no-op from cursor perspective)
    Continue,
    /// Jump cursor to target block
    Jump(Block, Vec<V>),
    /// Push a new frame for function call
    Call {
        callee: SpecializedFunction,
        args: Vec<V>,
        results: Vec<ResultValue>,
    },
    /// Pop current frame with return value
    Return(V),
    /// Yield from inline body execution
    Yield(V),
    /// Stop execution
    Stop(V),
    /// Delegate to dialect machine
    Delegate(E),
}
```

**Option B: The interpreter knows its own effect type**

Since the interpreter is parameterized by `R: Receipt` and the effect type is
derived from `R::Machine::Effect`, the interpreter can directly pattern-match.
For composite effects, the derive macro generates the match arms.

Option B is simpler for the MVP. The interpreter implementation knows the
concrete effect type and matches directly.

### Handling the unit effect

When `Interpretable::Effect = ()`, the interpreter should advance the cursor.
This is handled by the driver loop, not by `consume_effect`:

```rust
fn step(&mut self) -> Result<StepResult, Self::Error> {
    let stmt = self.current_statement()?;
    let effect = self.interpret_statement(stmt)?;

    // Unit effect → advance cursor
    // Non-unit effect → consume_effect
    self.consume_effect(effect)?;
    self.advance_cursor()?;

    Ok(StepResult::Stepped)
}
```

Actually, the advance-after-consume pattern depends on the effect. Jump effects
already set the cursor, so we shouldn't advance again. The cleanest approach:

```rust
fn step(&mut self) -> Result<StepResult, Self::Error> {
    let stmt = self.current_statement()?;

    // interpret_current dispatches to the dialect's Interpretable::interpret
    // and handles effect lifting + consumption internally
    self.interpret_and_apply(stmt)?;

    Ok(StepResult::Stepped)
}
```

Where `interpret_and_apply`:
1. Calls `Interpretable::interpret` on the current statement
2. If the returned effect is `()`, advances the cursor
3. If the returned effect is non-unit, calls `consume_effect` which updates
   cursor/frames as needed

## Driver Loop

The driver loop is similar to interpreter-2's:

```rust
fn run(&mut self) -> Result<RunResult, Self::Error> {
    loop {
        // Check suspension conditions
        if let Some(suspension) = self.check_suspension()? {
            return Ok(RunResult::Suspended(suspension));
        }

        // Get current statement
        let Some(stmt) = self.current_statement() else {
            return Ok(RunResult::Completed);
        };

        // Interpret and apply effects
        self.interpret_and_apply(stmt)?;

        // Check for stop
        if let Some(stop) = self.take_stop() {
            return Ok(RunResult::Stopped(stop));
        }

        // Burn fuel
        self.burn_fuel();
    }
}
```

### Suspension priority

1. Breakpoint at current location
2. Fuel exhausted
3. Host interrupt

These are checked BEFORE statement execution, matching interpreter-2's behavior.

## Stage Dispatch

Stage dispatch follows kirin-interpreter v1's approach. The interpreter holds
`&'ir Pipeline<R::StageInfo>` and dispatches based on the stage info.

For single-stage interpreters, `R::StageInfo = StageInfo<L>` and no dispatch is
needed — there's only one dialect.

For multi-stage interpreters, `R::StageInfo` is a stage enum implementing
`StageMeta` + `HasStageInfo<L>` for each dialect. The interpreter dispatches
using the existing tuple-reduction mechanism from kirin-ir.

The dispatch happens in `interpret_and_apply`:

```rust
fn interpret_and_apply(&mut self, stmt: Statement) -> Result<(), Self::Error> {
    // Read the statement's dialect from IR
    // Dispatch to the appropriate L's Interpretable::interpret
    // Lift the local effect to the top-level effect
    // Consume the top-level effect
}
```

For single-stage, this is a direct call. For multi-stage, this uses the stage
dispatch cache from kirin-interpreter v1.

## Receipt Examples

### Single-dialect concrete execution

```rust
struct ArithReceipt;

impl Receipt for ArithReceipt {
    type Language = ArithLanguage;
    type Value = i64;
    type Machine = (); // no dialect machine state
    type StageInfo = StageInfo<ArithLanguage>;
    type Error = InterpreterError;
}
```

### Multi-dialect concrete execution

```rust
struct MyAppReceipt;

impl Receipt for MyAppReceipt {
    type Language = MyLanguage;
    type Value = MyValue;
    type Machine = MyMachine;
    type StageInfo = MyStageEnum;
    type Error = MyError;
}
```

### Testing pattern

```rust
#[test]
fn test_add_semantics() {
    let mut pipeline = Pipeline::new();
    // ... build IR ...

    let mut interp = ConcreteInterpreter::<ArithReceipt>::new(
        &pipeline,
        stage_id,
    );

    // Seed values
    interp.write_ssa(lhs, 10)?;
    interp.write_ssa(rhs, 20)?;

    // Execute one statement
    interp.step()?;

    // Check result
    assert_eq!(interp.read(result)?, 30);
}
```

## Deferred Topics

- `AbstractInterpreter<R>` with fixpoint execution seeds
- Derive macros for `Receipt`, `Machine` composition, `Interpretable`
- Dynamic interpreter (multi-stage with heterogeneous value types)
- Graph execution seeds (DiGraph, UnGraph)
- Callee query builder (builder pattern)
- Driver control traits (Fuel, Breakpoints, Interrupt) — same as interpreter-2
- Position trait for read-only cursor inspection
