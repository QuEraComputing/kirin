# Kirin

Kirin is the *K*ernel *I*ntermediate *R*epresentation *In*frastructure. It is a compiler
infrastructure for building compilers for domain-specific languages (DSLs) that target
scientific computing kernels.

## Features

- MLIR-like dialects as composable python packages
- Generated Python frontend for your DSLs
- Pythonic API for building compiler passes
- Julia-like abstract interpretation framework
- Builtin support for interpretation
- Builtin support Python type system and type inference
- Type hinted via modern Python type hints

## Kirin's mission

Compiler toolchain for scientists. Scientists are building domain-specific languages (DSLs) for
scientific purposes. These DSLs are often high-level, and their instructions are usually slower
than the low-level instructions and thus result in smaller programs. The performance does not need
to be as good as a native program, but scientists want good interactivity and fast prototyping.
Most importantly, scientists usually just want to write Python as their frontend.

## Quick Example: the `beer` language

In this example, we will mutate python's semantics to
support a small eDSL (embedded domain-specific language) called `beer`.
It describes the process of brewing beer and get drunk.

First, let's define the [dialect](/def/#dialects) object, which is a registry for all
the objects modeling the semantics.

```python
from kirin import ir

dialect = ir.Dialect("beer")
```

Next, we want to define a runtime value `Beer` for the `beer` language so that we may use
later in our interpreter.

```python
from dataclasses import dataclass

@dataclass
class Beer:
    brand: str
```

Now, we can define the `beer` language's [statements](/def/#statements).

```python
from kirin.decl import statement, info
from kirin.dialects.py import types

@statement(dialect=dialect)
class NewBeer(Statement):
    name = "new_beer"
    traits = frozenset({ir.Pure()})
    brand: ir.SSAValue = info.argument(types.String)
    result: ir.ResultValue = info.result(types.PyClass(Beer))
```

the `NewBeer` statement creates a new beer object with a given brand. Thus
it takes a string as an argument and returns a `Beer` object. The `name` field
specifies the name of the statement in the IR text format (e.g printing). The
`traits` field specifies the statement's traits, in this case, it is a [pure
function](/101/#what-is-purity) because each brand name uniquely identifies a
beer object.

```python
@statement(dialect=dialect)
class Drink(Statement):
    name = "drink"
    beverage: SSAValue = info.argument(types.PyClass(Beer))


@statement(dialect=dialect)
class Pour(Statement):
    name = "pour"
    beverage: SSAValue = info.argument(types.PyClass(Beer))
    amount: SSAValue = info.argument(types.Int)


@statement(dialect=dialect)
class RandomBranch(Statement):
    name = "random_br"
    traits = frozenset({IsTerminator()})
    cond: SSAValue = info.argument(types.Bool)
    then_arguments: tuple[SSAValue, ...] = info.argument()
    else_arguments: tuple[SSAValue, ...] = info.argument()
    then_successor: Block = info.block()
    else_successor: Block = info.block()


@statement(dialect=dialect)
class Puke(Statement):
    name = "puke"
```
