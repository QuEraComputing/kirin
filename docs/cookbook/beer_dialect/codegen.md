## Codegen Food receipt

At the end of the day, we enjoy the food, take a nap, but still need to pay the bill.
In this section we will use the previous food fee analysis result, and discuss how to using kirin's codegen framework to generate a receipt.

### Goal

Lets again continue with the same program, and using the previous `FeeAnalysis` to get analysis result.
```python
@food
def main2(x: int):

    burger = NewFood(type="burger")
    salad = NewFood(type="salad")

    burger_serving = Cook(burger, 12 + x)
    salad_serving = Cook(salad, 10 + x)

    Eat(burger_serving)
    Eat(salad_serving)
    Nap()

    Eat(burger_serving)
    Nap()

    Eat(burger_serving)
    Nap()

    return x
```

We want to generate a recept of bill that listed the type of food cooked, and the amount of servings that were cooked.

### Codegen using kirin EmitStr
Kirin also provide Codegen framework (we call it Emit), which is also a kind of `Interpreter`!

Here, since we want to codegen recept in text format, our target is `Str`. We will use a `EmitStr` kirin provide. In general one can also customize the Codegen by customizing `EmitABC`, but here we will just directly using `EmitStr` provided by kirin.

```python
def default_menu_price():
    return {
        "burger": 3.0,
        "salad": 4.0,
        "chicken": 2.0,
    }



@dataclass
class EmitReceptMain(EmitStr):
    keys = ["emit.recept"]
    dialects: ir.DialectGroup = field(default=food)
    file: StringIO = field(default_factory=StringIO)
    menu_price: dict[str, float] = field(default_factory=default_menu_price)
    recept_analysis_result: dict[ir.SSAValue, Item] = field(default_factory=dict)

    def initialize(self):
        super().initialize()
        self.file.truncate(0)
        self.file.seek(0)
        return self

    def eval_stmt_fallback(
        self, frame: EmitStrFrame, stmt: ir.Statement
    ) -> tuple[str, ...]:
        return (stmt.name,)

    def emit_block(self, frame: EmitStrFrame, block: ir.Block) -> str | None:
        for stmt in block.stmts:
            result = self.eval_stmt(frame, stmt)
            if isinstance(result, tuple):
                frame.set_values(stmt.results, result)
        return None

    def get_output(self) -> str:
        self.file.seek(0)
        return "\n".join(
            [
                "item    \tamount \t  price",
                "-----------------------------------",
                self.file.read(),
            ]
        )
```

The same as all the other kirin interpreters, we need to implement MethodTable for our emit interpreter. Here, we register method tables to key `emit.recept`.

```python
@func.dialect.register(key="emit.recept")
class FuncEmit(interp.MethodTable):

    @interp.impl(func.Function)
    def emit_func(self, emit: EmitReceptMain, frame: EmitStrFrame, stmt: func.Function):
        _ = emit.run_ssacfg_region(frame, stmt.body)
        return ()
```

For our `Cook` Statement, we want to generate a transaction each time we cook. We will get the previous analysis result from the corresponding SSAValue. If the lattce element is a `AtLeastXItem`, we generate a line with the food type, and `>= x`. If its a `ConstIntItem` we just directly generate the amount.

```python
@dialect.register(key="emit.recept")
class FoodEmit(interp.MethodTable):

    @interp.impl(stmts.Cook)
    def emit_cook(self, emit: EmitReceptMain, frame: EmitStrFrame, stmt: stmts.Cook):
        serving_item = cast(ItemServing, emit.recept_analysis_result[stmt.result])

        amount_str = ""
        price_str = ""
        if isinstance(serving_item.count, AtLeastXItem):
            amount_str = f">={serving_item.count.data}"
            price_str = (
                f"  >=${emit.menu_price[serving_item.type] * serving_item.count.data}"
            )
        elif isinstance(serving_item.count, ConstIntItem):
            amount_str = f"  {serving_item.count.data}"
            price_str = (
                f"  ${emit.menu_price[serving_item.type] * serving_item.count.data}"
            )
        else:
            raise EmitError("invalid analysis result.")

        emit.writeln(frame, f"{serving_item.type}\t{amount_str}\t{price_str}")

        return ()
```

## Put together:

```python
emitter = EmitReceptMain()
emitter.recept_analysis_result = results

emitter.run(main2, ("",))
print(emitter.get_output())
```
