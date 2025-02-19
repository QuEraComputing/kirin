## Rewrite if-else control flow

In the main page, we introduce a simple `food` dialect example, and described how to use kirin to define a simple compiler.
In this section, we want to continue with this exampe, and consider a more compilcated rewrite pass
that involves the build-in python dialect if-else control flow.

### Goal
When one get really really full, not only does one nap, we also would make random decision.
Here specifically, We want to rewrite the existing `IfElse` statement defined in the existing `py` dialect into a customize `RandomBranch` statement we defined in our food dialect.

The execution of `RandomBranch`, as stated in its name, randomly execute a branch each time we run it.

### Define Custom RandomBranch statement
Lets start by define our `RandomBranch` Statement:

```python
from kirin.decl import statement, info
from kirin import ir, types

@statement(dialect=dialect)
class RandomBranch(ir.Statement):
    name = "random_br"
    traits = frozenset({ir.IsTerminator()}) # (1)!
    cond: SSAValue = info.argument(types.Bool) # (2)!
    then_arguments: tuple[ir.SSAValue, ...] = info.argument() # (3)!
    else_arguments: tuple[ir.SSAValue, ...] = info.argument() # (4)!
    then_successor: ir.Block = info.block() # (5)!
    else_successor: ir.Block = info.block() # (6)!
```

1. The `traits` field specifies that this statement is a terminator. A terminator is a statement that
   ends a block. In this case, the `RandomBranch` statement is a terminator because it decides which
   block to go next.
2. The `cond` field specifies the condition of the branch. It is a boolean value.
3. The `then_arguments` field specifies the arguments that are passed to the `then_successor` block. Unlike
   previous examples, the `then_arguments` field is annotated with `tuple[ir.SSAValue, ...]`, which means
   it takes a tuple of `ir.SSAValue` objects (like what it means in a `dataclass`).
4. The `else_arguments` field specifies the arguments that are passed to the `else_successor` block.
5. The `then_successor` field specifies the block that the control flow goes to if the condition is true.
6. The `else_successor` field specifies the block that the control flow goes to if the condition is false.

The `RandomBranch` statement is a terminator that takes a boolean condition and two tuples of arguments. However,
unlike a normal `if else` branching statement, it does not execute the branches based on the condition. Instead,
it randomly chooses one of the branches to execute. We will implement the execution behavior of this statement in the following.

### Implementation and MethodTable
Recall in the introduction of food dialect we metioned about `MethodTable`. Now we have defined the statement, we will need to tell interpreter how to interprete this Statement we defined.

Let's find the `FoodMethods` MethodTable that we defined and registered to `food` dialect previously:
```python
from kirin.interp import Frame, Successor, Interpreter, MethodTable, impl

@dialect.register
class FoodMethods(MethodTable):
    ...
```

Now we want to also implement the execution method and then register to this method table:

```python
    @impl(RandomBranch)
    def random_branch(self, interp: Interpreter, stmt: RandomBranch, values: tuple):
        frame = interp.state.current_frame()
        if randint(0, 1):
            return Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
        else:
            return Successor(
                stmt.else_successor, *frame.get_values(stmt.then_arguments)
            )
```

The `random_branch` implementation randomly chooses one of the branches to execute. The return value
is a [`Successor`][kirin.interp.Successor] object that specifies the next block to execute and the arguments
to pass to the block.

### Rewrite Python `if else` statement to `RandomBranch`

Now we can define a rewrite pass that rewrites Python `if else` statement to `RandomBranch` statement.
This is done by defining a subclass of [`RewriteRule`][kirin.rewrite.RewriteRule] and implementing the
`rewrite_Statement` method. The `RewriteRule` class is a standard Python visitor on Kirin's IR.

Here, we only need to implement the `rewrite_Statement` method to rewrite the `if else` statement to `RandomBranch`.

```python
from kirin.dialects import cf # (1)!
from kirin.rewrite import RewriteResult, RewriteRule # (2)!

@dataclass
class RewriteToRandomBranch(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult: # (3)!
        if not isinstance(node, cf.ConditionalBranch): # (4)!
            return RewriteResult()
        node.replace_by(
            RandomBranch(
                cond=node.cond,
                then_arguments=node.then_arguments,
                then_successor=node.then_successor,
                else_arguments=node.else_arguments,
                else_successor=node.else_successor,
            )
        )
        return RewriteResult(has_done_something=True) # (5)!
```

1. Import the control flow dialect `cf` which is what Python `if else` statement compiles to by default in the `basic` dialect group.
2. Import the `RewriteRule` class from the `rewrite` module.
3. This is the signature of `rewrite_Statement` method. Your IDE should hint you the type signature so you can auto-complete it.
4. Check if the statement is a `ConditionalBranch` statement. If it is not, return an empty `RewriteResult`.
5. Replace the `ConditionalBranch` statement with a `RandomBranch` statement and return a `RewriteResult` that indicates the rewrite has been done. Every statement has a [`replace_by`][kirin.ir.Statement.replace_by] method that replaces the statement with another statement.


### Adding to the decorator

```python
from kirin.ir import dialect_group
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint

@dialect_group(basic_no_opt.add(dialect))
def food(self):

    # some initialization if you need it
    def run_pass(mt, hungry:bool=False, got_lost: bool=True): # (1)!

        if drunk:
            Walk(NewFoodAndNap()).rewrite(mt.code)

        if got_lost:
            Fixpoint(Walk(RandomWalkBranch())).rewrite(mt.code) # (2)!

    return run_pass
```

1. Lets add an extra `got_lost` option to toggle this `RandomWalkBranch()` rewrite rule.
2. The `Walk` will walk through the IR and apply the rule. The `Fixpoint` then repeatedly walk through the IR until there is nothing to rewrite.
