# Toy Language Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an example binary (`toy-lang`) that reads `.kirin` files, parses them into IR, and interprets functions end-to-end.

**Architecture:** Two-stage pipeline (source/lowered) with two language enums. HighLevel uses structured CF + lexical lambdas (inline fields for E0275). LowLevel uses unstructured CF + lifted functions. Both need manual `Interpretable`/`CallSemantics` impls since inline variants prevent `#[derive(Interpretable)]`. The interpreter uses `StackInterpreter<i64, _>` (not `ArithValue` — `i64` has all the required trait impls: `BranchCondition`, `CompareValue`, `ForLoopValue`, arithmetic ops, bitwise ops).

**Tech Stack:** `clap` (derive), `anyhow`, `kirin` (with `interpreter` feature), dialect crates with `interpret` feature.

---

### Task 1: Scaffold the binary crate

**Files:**
- Create: `example/toy-lang/Cargo.toml`
- Create: `example/toy-lang/src/main.rs`
- Modify: `Cargo.toml` (root) — add workspace member + example entry

**Step 1: Add workspace member**

In root `Cargo.toml`, add `"example/toy-lang"` to `[workspace] members` array. Also add `clap` and `anyhow` to `[workspace.dependencies]`:

```toml
clap = { version = "4", features = ["derive"] }
anyhow = "1"
```

**Step 2: Create Cargo.toml**

```toml
[package]
name = "toy-lang"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "toy-lang"
path = "src/main.rs"

[dependencies]
kirin = { workspace = true, features = ["interpreter"] }
kirin-arith = { workspace = true, features = ["interpret"] }
kirin-bitwise = { workspace = true, features = ["interpret"] }
kirin-cf = { workspace = true, features = ["interpret"] }
kirin-cmp = { workspace = true, features = ["interpret"] }
kirin-constant = { workspace = true, features = ["interpret"] }
kirin-function = { workspace = true, features = ["interpret"] }
kirin-scf = { workspace = true, features = ["interpret"] }
kirin-interpreter = { workspace = true }
clap = { workspace = true }
anyhow = { workspace = true }
smallvec = { workspace = true }
```

**Step 3: Create minimal main.rs**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "toy-lang")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse a .kirin file and pretty-print the IR
    Parse {
        /// Path to the .kirin file
        file: std::path::PathBuf,
    },
    /// Parse and interpret a function
    Run {
        /// Path to the .kirin file
        file: std::path::PathBuf,
        /// Stage name (e.g. "source" or "lowered")
        #[arg(long)]
        stage: String,
        /// Function name (e.g. "main")
        #[arg(long, value_name = "NAME")]
        function: String,
        /// Arguments to the function (parsed as i64)
        args: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse { file } => {
            println!("TODO: parse {}", file.display());
        }
        Command::Run { file, stage, function, args } => {
            println!("TODO: run {} @{} @{} {:?}", file.display(), stage, function, args);
        }
    }
    Ok(())
}
```

**Step 4: Verify it builds**

Run: `cargo build -p toy-lang`
Expected: BUILD SUCCESS

**Step 5: Commit**

```
feat(toy-lang): scaffold binary crate with CLI skeleton
```

---

### Task 2: Define HighLevel language (parser + printer only)

**Files:**
- Create: `example/toy-lang/src/language.rs`

**Context:** HighLevel uses structured CF (if, for, yield), lexical lambdas, and wraps Arith/Cmp/Bitwise/Constant/Call/Return. Block/Region-containing variants are inlined (E0275 workaround). Lambda contains a Region. If/For contain Blocks.

**Step 1: Write HighLevel enum**

```rust
use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Call, Return};

/// Source-stage language: structured control flow + lexical lambdas.
///
/// Block/Region-containing types (If, For, Lambda) are inlined to avoid
/// E0275 trait recursion overflow with `#[wraps]` + `HasParser`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum HighLevel {
    // --- Region/Block-containing variants (inlined) ---
    #[chumsky(format = "{body}")]
    Function { body: Region },

    #[chumsky(format = "{res:name} = {.lambda} {name} captures({captures}) {body} -> {res:type}")]
    Lambda {
        name: Symbol,
        captures: Vec<SSAValue>,
        body: Region,
        res: ResultValue,
    },

    #[chumsky(format = "{.if} {condition} then {then_body} else {else_body}")]
    If {
        condition: SSAValue,
        then_body: Block,
        else_body: Block,
    },

    #[chumsky(format = "{.for} {induction_var} in {start}..{end} step {step} do {body}")]
    For {
        induction_var: SSAValue,
        start: SSAValue,
        end: SSAValue,
        step: SSAValue,
        body: Block,
    },

    #[kirin(terminator)]
    #[chumsky(format = "{.yield} {value}")]
    Yield { value: SSAValue },

    // --- Wrapped dialect variants ---
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
    #[wraps]
    Bitwise(Bitwise<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p toy-lang`
Expected: BUILD SUCCESS

**Step 3: Commit**

```
feat(toy-lang): define HighLevel language enum
```

---

### Task 3: Define LowLevel language (parser + printer only)

**Files:**
- Modify: `example/toy-lang/src/language.rs`

**Step 1: Add LowLevel enum**

```rust
use kirin_cf::ControlFlow;
use kirin_function::{Bind, FunctionBody};

/// Lowered-stage language: unstructured CF + lifted functions.
///
/// Function body is inlined (Region field, E0275 workaround).
/// All other variants use `#[wraps]` delegation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum LowLevel {
    #[chumsky(format = "{body}")]
    Function { body: Region },

    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
    #[wraps]
    Bitwise(Bitwise<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Cf(ControlFlow<ArithType>),
    #[wraps]
    Bind(Bind<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p toy-lang`
Expected: BUILD SUCCESS

**Step 3: Commit**

```
feat(toy-lang): define LowLevel language enum
```

---

### Task 4: Define Stage enum and implement `parse` subcommand

**Files:**
- Create: `example/toy-lang/src/stage.rs`
- Modify: `example/toy-lang/src/main.rs`

**Step 1: Write stage.rs**

```rust
use kirin::prelude::*;
use kirin::pretty::PipelinePrintExt;

use crate::language::{HighLevel, LowLevel};

#[derive(Debug, StageMeta, RenderDispatch)]
pub enum Stage {
    #[stage(name = "source")]
    Source(StageInfo<HighLevel>),
    #[stage(name = "lowered")]
    Lowered(StageInfo<LowLevel>),
}
```

**Step 2: Implement parse subcommand in main.rs**

```rust
mod language;
mod stage;

use kirin::prelude::*;
use kirin::pretty::PipelinePrintExt;

use stage::Stage;

// ... (CLI definition same as Task 1)

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse { file } => {
            let src = std::fs::read_to_string(&file)?;
            let mut pipeline: Pipeline<Stage> = Pipeline::new();
            pipeline.parse(&src)?;
            let rendered = pipeline.sprint();
            print!("{rendered}");
            Ok(())
        }
        Command::Run { .. } => {
            anyhow::bail!("run subcommand not yet implemented");
        }
    }
}
```

**Step 3: Write a test program**

Create `example/toy-lang/programs/add.kirin`:

```
stage @source fn @main(i64, i64) -> i64;

specialize @source fn @main(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
```

**Step 4: Test parsing**

Run: `cargo run -p toy-lang -- parse example/toy-lang/programs/add.kirin`
Expected: Pretty-printed IR output (no errors)

**Step 5: Commit**

```
feat(toy-lang): implement parse subcommand with Stage enum
```

---

### Task 5: Implement Interpretable for HighLevel

**Files:**
- Create: `example/toy-lang/src/interpret.rs`

**Context:** `#[derive(Interpretable)]` requires ALL variants to be `#[wraps]`. HighLevel has inline variants (Function, Lambda, If, For, Yield), so we need a manual impl. The manual impl delegates to existing `Interpretable` impls for wrapped types and reimplements the logic for inline types (copying from kirin-scf and kirin-function interpret_impl.rs).

**Step 1: Write manual Interpretable for HighLevel**

```rust
use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Shl, Shr, Sub};

use kirin::prelude::*;
use kirin_arith::{ArithType, ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::Bitwise;
use kirin_cmp::{Cmp, CompareValue};
use kirin_constant::Constant;
use kirin_function::{Call, Return};
use kirin_interpreter::{
    BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError,
};
use kirin_scf::ForLoopValue;
use smallvec::smallvec;

use crate::language::HighLevel;

impl<'ir, I> Interpretable<'ir, I, HighLevel> for HighLevel
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + Add<Output = I::Value>
        + Sub<Output = I::Value>
        + Mul<Output = I::Value>
        + CheckedDiv
        + CheckedRem
        + Neg<Output = I::Value>
        + CompareValue
        + BitAnd<Output = I::Value>
        + BitOr<Output = I::Value>
        + BitXor<Output = I::Value>
        + Not<Output = I::Value>
        + Shl<Output = I::Value>
        + Shr<Output = I::Value>
        + BranchCondition
        + ForLoopValue
        + From<ArithValue>,
    I::StageInfo: HasStageInfo<HighLevel>,
    I::Error: From<InterpreterError>,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            // Function body: jump to entry block
            HighLevel::Function { body } => {
                let stage = interp.resolve_stage::<HighLevel>()?;
                let entry = body
                    .blocks(stage)
                    .next()
                    .ok_or(InterpreterError::missing_entry_block())?;
                Ok(Continuation::Jump(entry, smallvec![]))
            }

            // Lambda: same as function body — jump to entry block
            HighLevel::Lambda { body, .. } => {
                let stage = interp.resolve_stage::<HighLevel>()?;
                let entry = body
                    .blocks(stage)
                    .next()
                    .ok_or(InterpreterError::missing_entry_block())?;
                Ok(Continuation::Jump(entry, smallvec![]))
            }

            // If: branch based on condition
            HighLevel::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond = interp.read(*condition)?;
                match cond.is_truthy() {
                    Some(true) => Ok(Continuation::Jump(*then_body, smallvec![])),
                    Some(false) => Ok(Continuation::Jump(*else_body, smallvec![])),
                    None => Ok(Continuation::Fork(smallvec![
                        (*then_body, smallvec![]),
                        (*else_body, smallvec![]),
                    ])),
                }
            }

            // For loop: iterate with induction variable
            HighLevel::For {
                start,
                end,
                step,
                body,
                ..
            } => {
                let mut iv = interp.read(*start)?;
                let end_val = interp.read(*end)?;
                let step_val = interp.read(*step)?;
                let stage = interp.active_stage_info::<HighLevel>();
                while iv.loop_condition(&end_val) == Some(true) {
                    interp.bind_block_args(stage, *body, &[iv.clone()])?;
                    let control = interp.eval_block(stage, *body)?;
                    match control {
                        Continuation::Yield(_) => {}
                        other => return Ok(other),
                    }
                    iv = iv.loop_step(&step_val);
                }
                Ok(Continuation::Continue)
            }

            // Yield: return value from SCF block
            HighLevel::Yield { value } => {
                let v = interp.read(*value)?;
                Ok(Continuation::Yield(v))
            }

            // Wrapped variants: delegate to inner type's Interpretable impl
            HighLevel::Arith(inner) => inner.interpret(interp),
            HighLevel::Cmp(inner) => inner.interpret(interp),
            HighLevel::Bitwise(inner) => inner.interpret(interp),
            HighLevel::Constant(inner) => inner.interpret(interp),
            HighLevel::Call(inner) => inner.interpret(interp),
            HighLevel::Return(inner) => inner.interpret(interp),
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p toy-lang`
Expected: BUILD SUCCESS

**Step 3: Commit**

```
feat(toy-lang): manual Interpretable impl for HighLevel
```

---

### Task 6: Implement CallSemantics for HighLevel

**Files:**
- Modify: `example/toy-lang/src/interpret.rs`

**Context:** `CallSemantics` defines how the interpreter enters a function body when processing a `Continuation::Call`. The `StackInterpreter` calls `L::eval_call(...)` where `L` is the language. For HighLevel, both `Function` and `Lambda` are callable bodies. We implement `SSACFGRegion` for HighLevel so it gets a blanket `CallSemantics` impl.

**Step 1: Implement SSACFGRegion for HighLevel**

```rust
use kirin_interpreter::SSACFGRegion;

impl SSACFGRegion for HighLevel {
    fn entry_block<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<Block, InterpreterError> {
        match self {
            HighLevel::Function { body } | HighLevel::Lambda { body, .. } => {
                body.blocks(stage)
                    .next()
                    .ok_or(InterpreterError::missing_entry_block())
            }
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p toy-lang`
Expected: BUILD SUCCESS

**Step 3: Commit**

```
feat(toy-lang): CallSemantics for HighLevel via SSACFGRegion
```

---

### Task 7: Implement Interpretable and CallSemantics for LowLevel

**Files:**
- Modify: `example/toy-lang/src/interpret.rs`

**Context:** LowLevel has only one inline variant (`Function { body: Region }`). All other variants are wrapped and have existing `Interpretable` impls. Same pattern as HighLevel but simpler.

**Step 1: Write manual Interpretable for LowLevel**

```rust
use crate::language::LowLevel;
use kirin_cf::ControlFlow;
use kirin_function::{Bind, FunctionBody};

impl<'ir, I> Interpretable<'ir, I, LowLevel> for LowLevel
where
    I: Interpreter<'ir>,
    I::Value: Clone
        + Add<Output = I::Value>
        + Sub<Output = I::Value>
        + Mul<Output = I::Value>
        + CheckedDiv
        + CheckedRem
        + Neg<Output = I::Value>
        + CompareValue
        + BitAnd<Output = I::Value>
        + BitOr<Output = I::Value>
        + BitXor<Output = I::Value>
        + Not<Output = I::Value>
        + Shl<Output = I::Value>
        + Shr<Output = I::Value>
        + BranchCondition
        + From<ArithValue>,
    I::StageInfo: HasStageInfo<LowLevel>,
    I::Error: From<InterpreterError>,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            LowLevel::Function { body } => {
                let stage = interp.resolve_stage::<LowLevel>()?;
                let entry = body
                    .blocks(stage)
                    .next()
                    .ok_or(InterpreterError::missing_entry_block())?;
                Ok(Continuation::Jump(entry, smallvec![]))
            }
            LowLevel::Arith(inner) => inner.interpret(interp),
            LowLevel::Cmp(inner) => inner.interpret(interp),
            LowLevel::Bitwise(inner) => inner.interpret(interp),
            LowLevel::Constant(inner) => inner.interpret(interp),
            LowLevel::Cf(inner) => inner.interpret(interp),
            LowLevel::Bind(inner) => inner.interpret(interp),
            LowLevel::Call(inner) => inner.interpret(interp),
            LowLevel::Return(inner) => inner.interpret(interp),
        }
    }
}
```

**Step 2: Implement SSACFGRegion for LowLevel**

```rust
impl SSACFGRegion for LowLevel {
    fn entry_block<L: Dialect>(&self, stage: &StageInfo<L>) -> Result<Block, InterpreterError> {
        match self {
            LowLevel::Function { body } => {
                body.blocks(stage)
                    .next()
                    .ok_or(InterpreterError::missing_entry_block())
            }
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}
```

**Step 3: Verify it compiles**

Run: `cargo build -p toy-lang`
Expected: BUILD SUCCESS

**Step 4: Commit**

```
feat(toy-lang): Interpretable and CallSemantics for LowLevel
```

---

### Task 8: Implement the `run` subcommand

**Files:**
- Modify: `example/toy-lang/src/main.rs`

**Context:** The `run` command parses a `.kirin` file, resolves the stage and function, then uses `StackInterpreter<i64, _>` to execute. Stage dispatch uses `StageMeta::declared_stage_names()` to validate, then matches on the stage name to select HighLevel or LowLevel. Arguments are parsed as `i64`.

**Step 1: Implement run_program function**

```rust
use kirin::interpreter::StackInterpreter;
use kirin_interpreter::StageAccess;

use stage::Stage;
use language::{HighLevel, LowLevel};

fn run_program(
    pipeline: &Pipeline<Stage>,
    stage_name: &str,
    func_name: &str,
    args: &[i64],
) -> anyhow::Result<i64> {
    // Find the stage by name
    let stage_id = pipeline
        .stages()
        .find(|&sid| {
            pipeline
                .stage(sid)
                .map(|s| s.stage_name().map(|n| n.as_str()) == Some(stage_name))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("stage @{stage_name} not found"))?;

    // Find the function by name
    let func_sym = pipeline
        .lookup_symbol(func_name)
        .ok_or_else(|| anyhow::anyhow!("function @{func_name} not found"))?;

    let func = pipeline
        .function_by_name(func_sym)
        .ok_or_else(|| anyhow::anyhow!("function @{func_name} not found"))?;

    let func_info = pipeline
        .function_info(func)
        .ok_or_else(|| anyhow::anyhow!("function @{func_name} has no info"))?;

    let staged_func = func_info
        .staged_functions()
        .get(&stage_id)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("function @{func_name} not specialized in @{stage_name}"))?;

    // Dispatch to the correct dialect based on stage name
    match stage_name {
        "source" => {
            let mut interp: StackInterpreter<i64, _> =
                StackInterpreter::new(pipeline, stage_id);
            let result = interp
                .in_stage::<HighLevel>()
                .call(staged_func, args)
                .map_err(|e| anyhow::anyhow!("interpreter error: {e:?}"))?;
            Ok(result)
        }
        "lowered" => {
            let mut interp: StackInterpreter<i64, _> =
                StackInterpreter::new(pipeline, stage_id);
            let result = interp
                .in_stage::<LowLevel>()
                .call(staged_func, args)
                .map_err(|e| anyhow::anyhow!("interpreter error: {e:?}"))?;
            Ok(result)
        }
        _ => anyhow::bail!("unknown stage: @{stage_name}"),
    }
}
```

**Step 2: Wire into main**

```rust
Command::Run { file, stage, function, args } => {
    let src = std::fs::read_to_string(&file)?;
    let mut pipeline: Pipeline<Stage> = Pipeline::new();
    pipeline.parse(&src)?;

    let parsed_args: Vec<i64> = args
        .iter()
        .map(|a| a.parse::<i64>())
        .collect::<Result<_, _>>()?;

    let result = run_program(&pipeline, &stage, &function, &parsed_args)?;
    println!("{result}");
    Ok(())
}
```

**Step 3: Test with add.kirin**

Run: `cargo run -p toy-lang -- run example/toy-lang/programs/add.kirin --stage source --function main 3 5`
Expected: `8`

**Step 4: Commit**

```
feat(toy-lang): implement run subcommand with StackInterpreter
```

---

### Task 9: Write example programs

**Files:**
- Modify: `example/toy-lang/programs/add.kirin` (already created)
- Create: `example/toy-lang/programs/factorial.kirin`
- Create: `example/toy-lang/programs/branching.kirin`

**Context:** The SCF `for` loop doesn't support iter_args (accumulators), so factorial uses recursion via `if/else` + `call`. Comparisons return `i64` (0 or 1), which `BranchCondition` treats as falsy/truthy. `ret` inside if/else body works: the `Return` continuation propagates up through the interpreter.

**Step 1: Create factorial.kirin (recursive)**

```
stage @source fn @factorial(i64) -> i64;

specialize @source fn @factorial(i64) -> i64 {
  ^entry(%n: i64) {
    %one = constant 1 -> i64;
    %is_base = le %n, %one -> i64;
    if %is_base then ^then() {
      ret %one;
    } else ^else() {
      %n_minus_1 = sub %n, %one -> i64;
      %rec = call @factorial(%n_minus_1) -> i64;
      %result = mul %n, %rec -> i64;
      ret %result;
    };
  }
}
```

**Step 2: Create branching.kirin (absolute value)**

```
stage @source fn @abs(i64) -> i64;

specialize @source fn @abs(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    if %is_neg then ^then() {
      %negated = neg %x -> i64;
      ret %negated;
    } else ^else() {
      ret %x;
    };
  }
}
```

**Step 3: Test all programs**

Run each:
```bash
cargo run -p toy-lang -- run example/toy-lang/programs/add.kirin --stage source --function main 3 5
# Expected: 8

cargo run -p toy-lang -- run example/toy-lang/programs/factorial.kirin --stage source --function factorial 5
# Expected: 120

cargo run -p toy-lang -- run example/toy-lang/programs/branching.kirin --stage source --function abs 42
# Expected: 42

cargo run -p toy-lang -- run example/toy-lang/programs/branching.kirin --stage source --function abs -- -7
# Expected: 7
```

Also test parse roundtrip:
```bash
cargo run -p toy-lang -- parse example/toy-lang/programs/factorial.kirin
# Expected: valid IR output
```

**Step 4: Commit**

```
feat(toy-lang): add factorial and branching example programs
```

---

### Task 10: Write integration tests

**Files:**
- Create: `example/toy-lang/tests/e2e.rs`
- Modify: `example/toy-lang/Cargo.toml` (add dev-dependency)

**Step 1: Add assert_cmd dev-dependency**

In `example/toy-lang/Cargo.toml`:

```toml
[dev-dependencies]
assert_cmd = "2"
```

And in root `Cargo.toml` workspace dependencies:

```toml
assert_cmd = "2"
```

**Step 2: Write integration tests**

```rust
use assert_cmd::Command;

fn toy_lang() -> Command {
    Command::cargo_bin("toy-lang").unwrap()
}

#[test]
fn test_parse_add() {
    toy_lang()
        .args(["parse", "programs/add.kirin"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(predicates::str::contains("add"));
}

#[test]
fn test_run_add() {
    toy_lang()
        .args(["run", "programs/add.kirin", "--stage", "source", "--function", "main", "3", "5"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("8\n");
}

#[test]
fn test_run_factorial_5() {
    toy_lang()
        .args(["run", "programs/factorial.kirin", "--stage", "source", "--function", "factorial", "5"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("120\n");
}

#[test]
fn test_run_factorial_0() {
    toy_lang()
        .args(["run", "programs/factorial.kirin", "--stage", "source", "--function", "factorial", "0"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("1\n");
}

#[test]
fn test_run_abs_positive() {
    toy_lang()
        .args(["run", "programs/branching.kirin", "--stage", "source", "--function", "abs", "42"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn test_run_abs_negative() {
    toy_lang()
        .args(["run", "programs/branching.kirin", "--stage", "source", "--function", "abs", "--", "-7"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("7\n");
}

#[test]
fn test_run_missing_function() {
    toy_lang()
        .args(["run", "programs/add.kirin", "--stage", "source", "--function", "nonexistent", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .failure();
}

#[test]
fn test_run_missing_stage() {
    toy_lang()
        .args(["run", "programs/add.kirin", "--stage", "nonexistent", "--function", "main", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .failure();
}
```

**Step 3: Run tests**

Run: `cargo nextest run -p toy-lang`
Expected: ALL PASS

**Step 4: Commit**

```
test(toy-lang): add end-to-end integration tests
```

---

### Notes for the implementor

1. **Value type is `i64`, not `ArithValue`**: The interpreter uses `i64` because it implements all required traits (`BranchCondition`, `CompareValue`, `ForLoopValue`, arithmetic ops, bitwise ops, `From<ArithValue>`). `ArithValue` only has `From<ArithValue> for i64`, not the reverse.

2. **Comparison result types**: In the `.kirin` programs, comparisons like `le %n, %one -> i64` produce `i64` (0 or 1), not `bool`. The `CompareValue for i64` impl returns 0/1. The `BranchCondition for i64` impl treats nonzero as truthy.

3. **`ret` inside if/else bodies**: When `If::interpret()` returns `Jump(then_body)`, the interpreter evaluates that block. If it ends with `ret`, the `Return` continuation propagates up. Statements after the `if` in the parent block are unreachable but harmless.

4. **Function resolution for `call`**: The `Call<T>::interpret()` impl in kirin-function resolves the function by name through the pipeline's symbol table. The pipeline populated this table during parsing via `ParsePipelineText`.

5. **`Staged::call()` vs `run()`**: `call(staged_func, args)` handles frame setup, entry block resolution, and block argument binding. It's the high-level API. `run()` requires manual frame setup.

6. **`Bind` in LowLevel**: `Bind::interpret()` is in kirin-function. It creates a closure-like binding. For the initial examples we don't use lowered-stage programs yet, so this is present but not tested.

7. **Recursive calls**: The `Call` interpret impl resolves function names at runtime through the pipeline. For recursive calls (factorial calling itself), this works naturally — the function name resolves to the same specialization each time.
