## More advance example of beer dialect


### Define Custom RandomBranch statement


<!-- This will be different when we implement interpretation for a terminator:

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
to pass to the block. -->


### Rewrite IfElse

<!--
Now we can define a more complicated statement that involves control flow.

```python
@statement(dialect=dialect)
class RandomBranch(Statement):
    name = "random_br"
    traits = frozenset({IsTerminator()}) # (1)!
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

the `RandomBranch` statement is a terminator that takes a boolean condition and two tuples of arguments. However,
unlike a normal `if else` branching statement, it does not execute the branches based on the condition. Instead,
it randomly chooses one of the branches to execute. -->


### Adding to the decorator

```python
from kirin.ir import dialect_group
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint

@dialect_group(basic_no_opt.add(dialect))
def beer(self):

    # some initialization if you need it
    def run_pass(mt, drunk:bool=False, got_lost: bool=True): # (1)!

        if drunk:
            Walk(NewBeerAndPukeOnDrink()).rewrite(mt.code)

        if got_lost:
            Fixpoint(Walk(RandomWalkBranch())).rewrite(mt.code) # (2)!

    return run_pass
```

1. Lets add an extra `got_lost` option to toggle this `RandomWalkBranch()` rewrite rule.
2. The `Walk` will walk through the IR and apply the rule. The `Fixpoint` then repeatedly walk through the IR until there is nothing to rewrite.
