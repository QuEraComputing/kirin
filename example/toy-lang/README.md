# toy-lang

A minimal language built on the Kirin compiler infrastructure, demonstrating parsing, pretty-printing, and interpretation of a simple IR.

## Language Features

toy-lang composes several Kirin dialects into two compilation stages:

**Source stage** (`@source`): structured control flow with lexical scoping
- Arithmetic: `add`, `sub`, `mul`, `div`, `rem`, `neg`
- Comparison: `lt`, `le`, `gt`, `ge`, `eq`, `ne`
- Bitwise: `and`, `or`, `xor`, `not`, `shl`, `shr`
- Constants: `constant <value> -> <type>`
- Control flow: `if <cond> then ^then() { ... } else ^else() { ... }`
- Functions: `call @name(args...) -> <type>`, `ret <value>`

**Lowered stage** (`@lowered`): unstructured control flow with lifted functions
- Same arithmetic/comparison/bitwise/constant ops
- Block-based control flow: `br ^block(args...)`, `cond_br <cond> ^then(args...) ^else(args...)`

## Building

From the workspace root:

```sh
cargo build -p toy-lang
```

## Usage

### Parse and pretty-print

```sh
cargo run -p toy-lang -- parse programs/add.kirin
```

### Run a function

```sh
cargo run -p toy-lang -- run programs/add.kirin --stage source --function main 3 5
# Output: 8
```

```sh
cargo run -p toy-lang -- run programs/factorial.kirin --stage source --function factorial 5
# Output: 120
```

```sh
cargo run -p toy-lang -- run programs/branching.kirin --stage source --function abs -- -7
# Output: 7
```

Use `--` before negative arguments so they aren't parsed as flags.

## Example Programs

**`programs/add.kirin`** — adds two integers:
```
stage @source fn @main(i64, i64) -> i64;

specialize @source fn @main(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
```

**`programs/factorial.kirin`** — recursive factorial with structured `if`/`else`.

**`programs/branching.kirin`** — absolute value using conditional branching.

## Tests

```sh
cargo nextest run -p toy-lang
```

Runs end-to-end tests that invoke the CLI binary and check output.
