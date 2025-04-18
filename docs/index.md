<div align="center">
<picture>
  <img id="logo_light_mode" src="assets/logo-black-horizontal.svg" style="width: 70%" alt="Kirin Logo">
  <img id="logo_dark_mode" src="assets/logo-white-horizontal.svg" style="width: 70%" alt="Kirin Logo">
</picture>
<!--pad the following div a bit top-->
<div style="padding-top: -100px">
<h2>Kernel Intermediate Representation Infrastructure</h2>
</div>
</div>

Kirin is the **K**ernel **I**ntermediate **R**epresentation **In**frastructure developed. It is a compiler infrastructure for building compilers for embedded domain-specific languages (eDSLs) that target scientific computing kernels especially for quantum computing use cases where domain-knowledge in quantum computation is critical in the implementation of a compiler.

## Installation

```bash
pip install kirin-toolchain
```

See [Installation](install.md) for more details.

## Features

- [MLIR](https://mlir.llvm.org/)-like dialects as composable python packages
- Generated Python frontend for your DSLs
- Pythonic API for building compiler passes
- [Julia](https://julialang.org)-like abstract interpretation framework
- Builtin support for interpretation
- Builtin support Python type system and type inference
- Type hinted via modern Python type hints

## Kirin's mission

Kirin empowers scientists to build tailored embedded domain-specific languages (eDSLs) by adhering to three core principles:

1. **Scientists First** Kirin prioritizes enabling researchers to create compilers for scientific challenges. The toolchain is designed *by* and *for* domain experts, ensuring practicality and alignment with real-world research needs.

2. **Focused Scope** Unlike generic compiler frameworks, Kirin deliberately narrows its focus to scientific applications. It specializes in high-level, structurally oriented eDSLs—optimized for concise, kernel-style functions that form the backbone of computational workflows.

3. **Composability as a Foundation** Science thrives on interdisciplinary collaboration. Kirin treats composability — the modular integration of systems and components—as a first-class design principle. This ensures eDSLs and their compilers can seamlessly interact, mirroring the interconnected nature of scientific domains.

For the interested, please read the [Kirin blog post](blog/posts/2024-nov-11-mission.md) blog post for more details.

## Acknowledgement

While the mission and audience may be very different, Kirin has been deeply inspired by a few projects:

- [MLIR](https://mlir.llvm.org/), the concept of dialects and the way it is designed.
- [xDSL](https://github.com/xdslproject/xdsl), about how IR data structure & interpreter should be designed in Python.
- [Julia](https://julialang.org/), abstract interpretation, and certain design choices for scientific community.
- [JAX](https://jax.readthedocs.io/en/latest/) and [numba](https://numba.pydata.org/), the frontend syntax and the way it is designed.
- [Symbolics.jl](https://github.com/JuliaSymbolics/Symbolics.jl) and its predecessors, the design of rule-based rewriter.

Part of the work is also inspired in previous collaboration in [YaoCompiler](https://github.com/QuantumBFS/YaoCompiler.jl), thus we would like to thank [Valentin Churavy](https://github.com/vchuravy) and [William Moses](https://github.com/wsmoses) for early discussions around the compiler plugin topic. We thank early support of the YaoCompiler project from [Unitary Foundation](https://unitary.foundation/).

## Kirin and friends

While at the moment only us at [QuEra Computing Inc](https://quera.com) are actively developing Kirin and using it in our projects, we are open to collaboration and encourage contributions from the community! If you are using Kirin in your project, please let us know so we can add you to the list of projects using Kirin.

### Quantum Computing

Kirin has been used for building several eDSLs within [QuEra Computing](https://quera.com), including:

- [bloqade.qasm2](https://github.com/QuEraComputing/bloqade/tree/main/src/bloqade/qasm2) - uses Kirin to define an eDSL for the Quantum Assembly Language (QASM) 2.0. It demonstrates how to create multiple dialects, run custom analysis and rewrites, and generate code from the dialects (back to QASM 2.0 in this case).
- [bloqade.stim](https://github.com/QuEraComputing/bloqade/tree/main/src/bloqade/stim) - uses Kirin to define an eDSL for the [Stim](https://github.com/quantumlib/Stim/) language. It demonstrates how to create multiple dialects, run custom analysis and rewrites, and generate code from the dialects (back to Stim in this case).
- [bloqade.qBraid](https://github.com/QuEraComputing/bloqade/blob/main/src/bloqade/qbraid/lowering.py) - An example demonstrating how to lower from an existing representation into Kirin IR by using the visitor pattern.

We are in the process of open-sourcing more eDSLs built on top of Kirin and encourage you to keep an eye out for them!

## Quick Example: the `food` language

For the impatient, we prepare an example that requires no background knowledge in any specific domain. In this example, we will mutate python's semantics to support a small eDSL called `food`. It describes the process of cooking, eating food, and taking food naps after.

Before we start, let's take a look at what would our `food` language look like:

```python
@food
def main(x: int):
    food = NewFood(type="burger")  # (1)!
    serving = Cook(food, x)  # (2)!
    Eat(serving)  # (3)!
    Nap()  # (4)!

    return x + 1  # (5)!
```

1. The `NewFood` statement creates a new food object with a given type.
2. The `Cook` statement makes that food for `x` portions into a servings object.
3. The `Eat` statement means you eat a serving object.
4. The `Nap` statement means you nap. Eating food makes you sleepy!!
5. Doing some math to get a result.

The food language is wrapped with a decorator `@food` to indicate that the function is written in the `food` language instead of normal Python. (think about how would you program GPU kernels in Python, or how would you use `jax.jit` and `numba.jit` decorators).

You can run the `main` function as if it is a normal Python function.

```python
main(1)
```

or you can inspect the compiled result via

```python
main.print()
```

![food-printing](assets/food-printing.png)


### Defining the dialect
First, let's define the [dialect](def.md#dialects) object, which is a registry for all
the objects modeling the semantics.

```python
from kirin import ir

dialect = ir.Dialect("food")
```

### Defining the statements

Next, we want to define a runtime value `Food`, as well as the runtime value of `Servings` for the `food` language so that we may use
later in our interpreter. These are just a standard Python `dataclass`.

```python
from dataclasses import dataclass

@dataclass
class Food:
    type: str


@dataclass
class Serving:
    kind: Food
    amount: int
```

Now, we can define the `food` language's [statements](def.md#statements).

```python
from kirin.decl import statement, info
from kirin import ir, types

@statement(dialect=dialect)
class NewFood(ir.Statement):
    name = "new_food"
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    type: str = info.attribute(types.String)
    result: ir.ResultValue = info.result(types.PyClass(Food))
```

1. The `name` field specifies the name of the statement in the IR text format (e.g printing).
2. The `traits` field specifies the statement's traits, in this case, it is a
   [pure function](101.md/#what-is-purity) because each brand name uniquely identifies a
   food object. We also add a trait of `FromPythonCall()` to allow lowering from a Python call in the Python AST.
3. The `type` field specifies the argument of the statement. It is an Attribute of string value. See [`PyAttr`][kirin.ir.PyAttr] for further details.
4. The `result` field specifies the result of the statement. Usually a statement only has one result
   value. The type of the result must be [`ir.ResultValue`](def.md#ssa-values) with a field specifier
    `info.result` that optionally specifies the type of the result.

the `NewFood` statement creates a new food object with a given brand. Thus
it takes a string as an attribute and returns a `Food` object. Click the plus sign above
to see the corresponding explanation.


```python
@statement(dialect=dialect)
class Cook(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    target: ir.SSAValue = info.argument(types.PyClass(Food)) # (1)!
    amount: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(types.PyClass(Serving))

```

1. The arguments of a [`Statement`](def.md#statements) must be [`ir.SSAValue`](def.md#ssa-values) objects with a
   field specifier `info.argument` that optionally specifies the type of the argument.

Next, we define `Cook` statement that takes a `Food` object as an argument, and the result value is a `Serving` object. The `types.PyClass` type understands Python classes and can take a Python class as an argument to create a type attribute [`TypeAttribute`](def.md#attributes).


```python
@statement(dialect=dialect)
class Eat(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    target: ir.SSAValue = info.argument(types.PyClass(Serving))
```

Similarly, we define `Eat` statement that takes a `Serving` object as an argument. As the same previously, the `types.PyClass` type understands Python classes (in this case the `Serving` class) and can take a Python class as an argument to create a type attribute. Notice that `Eat` does not have any return value.

Finally, we define the `Nap` statement that describes the nap action, which does not have any arguments and has no return value.

```python
@statement(dialect=dialect)
class Nap(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
```


### Defining the method table for concrete interpreter

Now with the statements defined, we can define how to interpret them by defining the method table associate with each statement.

```python
from kirin.interp import Frame, Interpreter, MethodTable, impl

@dialect.register
class FoodMethods(MethodTable):
    ...

```

The `FoodMethods` class is a subclass of `MethodTable`. Together with the decorator from the dialect group `@dialect.register`, they register the implementation of the method table to interpreter. The implementation is a method decorated with `@impl` that executes the
statement.

```python
    @impl(NewFood)
    def new_food(self, interp: Interpreter, frame: Frame, stmt: NewFood):
        return (Food(stmt.type),) # (1)!

    @impl(Eat)
    def eat(self, interp: Interpreter, frame: Frame, stmt: Eat):
        serving: Serving = frame.get(stmt.target)
        print(f"Eating {serving.amount} servings of {serving.kind.type}")
        return ()

    @impl(Cook)
    def cook(self, interp: Interpreter, frame: Frame, stmt: Cook): # (2)!
        food: Food = frame.get(stmt.target)
        amount: int = frame.get(stmt.amount)
        print(f"Cooking {food.type} {amount}")

        return (Serving(food, amount),)

    @impl(Nap)
    def nap(self, interp: Interpreter, frame: Frame, stmt: Nap):
        print("Napping!!!")
        return () # (3)!
```

1. The statement has return value which is a `Food` runtime object.
2. Sometimes, the execution of a statement will have *side-effect* and return value.
For example, here the execution `Cook` statement print strings (side-effect) as well as return a `Serving` runtime object.
3. In the case where the statement does not have any return value but simply have side-effect only, the return value is simply an empty tuple.

The return value is just a normal tuple that contains interpretation runtime values. Click the plus sign above
to see the corresponding explanation.


### Rewrite `Eat` statement

Sometimes when we are hungry, we will do something that is not expected. Here, we introduce how to do a rewrite on the program.
What we want to do is simple:

Every time we eat, we will to buy another piece of food, then take a nap. *Someone has the munchies, eh?*


More specifically, we want to rewrite the program such that, every time we encounter a `Eat` statement, we insert a `NewFood` statement, and `Nap` after `Eat`.
Let's define a rewrite pass that rewrites our `Eat` statement. This is done by defining a subclass of [`RewriteRule`][kirin.rewrite.abc.RewriteRule] and implementing the
`rewrite_Statement` method. The `RewriteRule` class is a standard Python visitor on Kirin's IR.


```python
from kirin.rewrite.abc import RewriteRule # (1)!
from kirin.rewrite.result import RewriteResult
from kirin import ir

@dataclass
class NewFoodAndNap(RewriteRule):
    # sometimes someone is hungry and needs a nap
    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult: # (2)!
        if not isinstance(node, Eat): # (3)!
            return RewriteResult()

        # 1. create new stmts:
        new_food_stmt = NewFood(type="burger") # (4)!
        nap_stmt = Nap() # (5)!

        # 2. put them in the ir
        new_food_stmt.insert_after(node) # (6)!
        nap_stmt.insert_after(new_food_stmt)

        return RewriteResult(has_done_something=True) # (7)!

```

1. Import the `RewriteRule` class from the `rewrite` module.
2. This is the signature of the `rewrite_Statement` method. Your IDE should hint the type signature so you can auto-complete it.
3. Check if the statement is a `Eat` statement. If it is not, return an empty `RewriteResult`.
4. Create new `NewFood` statement.
5. Create new `Nap` statement.
6. insert the new created statements into the IR. Each of the ir.Statement provides an API such as [`insert_after`][kirin.ir.Statement.insert_after], [`insert_before`][kirin.ir.Statement.insert_after] and [`replace_by`][kirin.ir.Statement.replace_by] that allows you to insert a new statement either after or before, or replace the current statement with another one.
7. Return a `RewriteResult` that indicates the rewrite has been done.


### Putting everything together

Now we can put everything together and finally create the `food` decorator, and
you do not need to figure out the complicated type hinting and decorator implementation
because Kirin will do it for you!

```python
from kirin.ir import dialect_group
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk
from kirin.passes import Fold


@dialect_group(basic_no_opt.add(dialect)) # (1)!
def food(self): # (2)!

    fold_pass = Fold(self)

    def run_pass(mt, *, fold:bool=True, hungry:bool=True):  # (3)!

        if fold:
            fold_pass(mt)

        if hungry:
            Walk(NewFoodAndNap()).rewrite(mt.code) # (4)!

    return run_pass # (5)!
```

1. The [`dialect_group`][kirin.ir.group.dialect_group] decorator specifies the dialect group that the `food` dialect belongs to. In this case, instead of rebuilding the whole dialect group, we just add our `dialect` object to the [`basic_no_opt`][kirin.prelude.basic_no_opt] dialect group which provides all the basic Python semantics, such as math, function, closure, control flows, etc.
2. The `food` function is the decorator that will be used to decorate the `main` function.
3. The `run_pass` function wraps all the passes that need to run on the input method. It optionally can take some arguments or keyword arguments that will be passed to the `food` decorator.
4. Inside the `run_pass` function, we will traverse the entire IR and use the rule `NewFoodAndNap` to rewrite all the `Eat` statements.
5. Remember to return the `run_pass` function at the end of the `food` function.

This is it!

For further advanced use case see [`CookBook/Food`](cookbook/foodlang/cf_rewrite/)

## Contributors

- [QuEra Computing Inc](https://quera.com)

## License

Apache License 2.0 with LLVM Exceptions
