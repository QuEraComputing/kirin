## Codegen Beer receipt

At the end of the day, we enjoy the beers, get drunk, but still need to pay the bill.
In this section we will use the previous beer fee analysis result, and discuss how to using kirin's codegen framework to generate a receipt.

### Goal

Lets again continue with the same program, and using the previous `FeeAnalysis` to get analysis result.
```python
@beer
def main2(x: int):

    bud = NewBeer(brand="budlight")
    heineken = NewBeer(brand="heineken")

    bud_pints = Pour(bud, 12 + x)
    heineken_pints = Pour(heineken, 10 + x)

    Drink(bud_pints)
    Drink(heineken_pints)
    Puke()

    Drink(bud_pints)
    Puke()

    Drink(bud_pints)
    Puke()

    return x
```

We want to generate a recept of bill that listed the type of beer one pour, and how many pints one pour.

### Codegen using kirin EmitStr
Kirin also provide Codegen framework (we call it Emit), which is also a kind of `Interpreter`!

Here, since we want to codegen recept in text format, our target is `Str`. We will use a `EmitStr` kirin provide. In general one can also customize the Codegen by customizing `EmitABC`, but here we will just directly using `EmitStr` provided by kirin.

```python
def default_menu_price():
    return {
        "budlight": 3.0,
        "heineken": 4.0,
        "tsingdao": 2.0,
    }


@dataclass
class EmitReceptMain(EmitStr):
    keys = ["emit.recept"]
    dialects: ir.DialectGroup = field(default=beer)
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

For our `Pour` Statement, we want to generate a transaction each time we pour. We will get the previous analysis result from the corresponding SSAValue. If the lattce element is a `AtLeastXItem`, we generate a line with beer brand, and `>= x`. If its a `ConstIntItem` we just directly generate the amount.

```python
@dialect.register(key="emit.recept")
class BeerEmit(interp.MethodTable):

    @interp.impl(stmts.Pour)
    def emit_pour(self, emit: EmitReceptMain, frame: EmitStrFrame, stmt: stmts.Pour):
        pints_item: ItemPints = emit.recept_analysis_result[stmt.result]

        amount_str = ""
        price_str = ""
        if isinstance(pints_item.count, AtLeastXItem):
            amount_str = f">={pints_item.count.data}"
            price_str = (
                f"  >=${emit.menu_price[pints_item.brand] * pints_item.count.data}"
            )
        elif isinstance(pints_item.count, ConstIntItem):
            amount_str = f"  {pints_item.count.data}"
            price_str = (
                f"  ${emit.menu_price[pints_item.brand] * pints_item.count.data}"
            )
        else:
            raise EmitError("invalid analysis result.")

        emit.writeln(frame, f"{pints_item.brand}\t{amount_str}\t{price_str}")

        return ()
```

## Put together:

```python
emitter = EmitReceptMain()
emitter.recept_analysis_result = results

emitter.run(main2, ("",))
print(emitter.get_output())
```
