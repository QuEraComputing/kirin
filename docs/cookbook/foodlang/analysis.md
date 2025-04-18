## Food cost and Nap analysis

In this section we will discuss on how to perform analysis of a Kirin program. We will again use our `food` dialect example.

### Goal

Let's consider the following program

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

We would like to implement an forward dataflow analysis that walks through the program and collects the pricing information for each of the statements as well as how many times one has napped.

### Defining a Lattice
One of the important concepts related to doing static analysis is the *Lattice* (See [Wiki:Lattice](https://en.wikipedia.org/wiki/Lattice_(order)) and [Lecture Notes On Static Analysis](https://studwww.itu.dk/~brabrand/static.pdf) for further details)
A Lattice defines the partial order of the lattice element. An simple example is the type lattice.

Let's now defines our `Item` lattice for the price analysis.

First, a lattice always has `top` and `bottom` elements. In type lattice, the top element is `Any` and bottom element is `None`.

Here, we define `AnyItem` as `top` and `NoItem` as `bottom`. In Kirin, we can simply inherit the `BoundedLattice` from `kirin.lattice`. Kirin also provides some simple mixins with default implementations of the API such as `is_subseteq`, `join` and `meet` so you don't have to re-implement them.

```python
from kirin.lattice import (
    SingletonMeta,
    BoundedLattice,
    IsSubsetEqMixin,
    SimpleJoinMixin,
    SimpleMeetMixin,
)
from typing import final
from dataclasses import dataclass

@dataclass
class Item(
    IsSubsetEqMixin["Item"],
    SimpleJoinMixin["Item"],
    SimpleMeetMixin["Item"],
    BoundedLattice["Item"],
):

    @classmethod
    def top(cls) -> "Item":
        return AnyItem()

    @classmethod
    def bottom(cls) -> "Item":
        return NotItem()


@final
@dataclass
class NotItem(Item, metaclass=SingletonMeta): # (1)!
    """The bottom of the lattice.

    Since the element is the same without any field,
    we can use the SingletonMeta to make it a singleton by inheriting from the metaclass

    """

    def is_subseteq(self, other: Item) -> bool:
        return True


@final
@dataclass
class AnyItem(Item, metaclass=SingletonMeta):
    """The top of the lattice.

    Since the element is the same without any field,
    we can use the SingletonMeta to make it a singleton by inheriting from the metaclass

    """

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AnyItem)

```

1. Notice that since `NotItem` and `AnyItem` do not have any properties, we can mark them as singletons to prohibit duplicate copies of instances by inheriting from the `SingletonMeta` metaclass.

Next there are a few more lattice elements we want to define:

```python
@final
@dataclass
class ItemServing(Item): # (1)!
    count: Item
    type: str

    def is_subseteq(self, other: Item) -> bool:
        return (
            isinstance(other, ItemServing)
            and self.count == other.count
            and self.type == other.type
        )


@final
@dataclass
class AtLeastXItem(Item): # (2)!
    data: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AtLeastXItem) and self.data == other.data


@final
@dataclass
class ConstIntItem(Item): # (3)!
    data: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, ConstIntItem) and self.data == other.data


@final
@dataclass
class ItemFood(Item): # (4)!
    type: str

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, ItemFood) and self.type == other.type
```

1. `ItemServing` which contains information of the kind of food of the `Serving`, as well as the count
2. `AtLeastXItem` which contains information of a constant type result value is a number that is least `x`. The `data` contain the lower-bound
3. `ConstIntItem` which contains a concrete number.
4. `ItemFood` which contains information of `Food`.


### Custom Forward Data Flow Analysis

Now we have defined our lattice. Let's move on to see how we can write an analysis pass, and get the analysis results.

In Kirin, the analysis pass is implemented with `AbstractInterpreter` (inspired by the [Julia programming language](https://julialang.org/)). Kirin provides an simple forward dataflow analysis `Forward`. So we will use that.

Here our analysis will do two things.

1. Get all the analysis results as a dictionary of SSAValue to lattice elements.
2. Count how many times one naps.

```python
from kirin.analysis import Forward, ForwardFrame
from kirin.interp import SpecialValue
from kirin.ir import Method

from dataclasses import field

@dataclass
class FeeAnalysis(Forward[Item]): # (1)!
    keys = ["food.fee"] # (2)!
    lattice = Item
    nap_count: int = field(init=False)

    def initialize(self): # (3)!
        """Initialize the analysis pass.

        The method is called before the analysis pass starts.

        Note:
            1. Here one is *required* to call the super().initialize() to initialize the analysis pass,
            which clears all the previous analysis results and symbol tables.
            2. Any additional initialization that belongs to the analysis should also be done here.
            For example, in this case, we initialize the `nap_count` to 0.

        """
        super().initialize()
        self.nap_count = 0
        return self

    def eval_stmt_fallback( # (4)!
        self, frame: ForwardFrame[Item], stmt: ir.Statement
    ) -> tuple[Item, ...] | SpecialValue[Item]:
        return ()

    def run_method(self, method: Method, args: tuple[Item, ...]) -> Item: # (5)!
        return self.run_callable(method.code, (self.lattice.bottom(),) + args)

```

1. Inherit from `Forward` with our custom lattice element `Item`.
2. The keys for the MethodTable. Remember that in Kirin all the implementation methods of an interpreter are implemented and registered to a `MethodTable`.
3. `AbstractInterpreter` has an abstract method `initialize` which will be called every time `run()` is called. We can overload this to reset the variable, so we can re-use this class instance.
4. This method implements a *fallback* for when a statement is encountered that has no corresponding method for it in the `MethodTable`. Here, we just return an empty tuple because we know all the statements that have a return value will be implemented, so only statements without return values will have to fall back.
5. This method defines and customizes how to run the `ir.Method`.

Click the + logo to see more details.

Now we want to implement how the statement gets run. This is the same as what we described when we mentioned the concrete interpreter.

Note that each dialect can have multiple registered `MethodTable`s, distinguished by a `key`. The interpreter uses the `key` to find the corresponding `MethodTable`s for each dialect in a dialect group.

Here, we use `key="food.fee"`

First we need to implement for `Constant` statement in `py.constant` dialect. If its `int`, we return `ConstIntItem` lattice element. If its `Food`, we return `ItemFood`.

```python
from kirin.dialects import py
from kirin.interp import MethodTable
from kirin.exceptions import InterpreterExit

@py.constant.dialect.register(key="food.fee")
class PyConstMethodTable(MethodTable):

    @interp.impl(py.constant.Constant)
    def const(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[Item],
        stmt: py.constant.Constant,
    ):
        if isinstance(stmt.value, int):
            return (ConstIntItem(data=stmt.value),)
        elif isinstance(stmt.value, Food):
            return (ItemFood(type=stmt.value.type),)

        else:
            raise InterpreterExit(
                f"illegal constant type {type(stmt.value)}"
            )
```


Next, since we allow the `add` operation in the program (Note the `12 + x` and `10 + x` operations in used in instantiating `Cook`), we also need to let abstract interpreter know how to interpret the `binop.Add` statement, which is in the `py.binop` dialect.

```python
@binop.dialect.register(key="food.fee")
class PyBinOpMethodTable(MethodTable):

    @interp.impl(binop.Add)
    def add(
        self,
        interp: FeeAnalysis,
        frame: Frame[Item],
        stmt: binop.Add,
    ):
        left = frame.get(stmt.lhs)
        right = frame.get(stmt.rhs)

        if isinstance(left, AtLeastXItem) or isinstance(right, AtLeastXItem):
            out = AtLeastXItem(data=left.data + right.data)
        else:
            out = ConstIntItem(data=left.data + right.data)

        return (out,)
```

Finally, we need an implementation for our food dialect statements.

```python
@dialect.register(key="food.fee")
class FoodMethodTable(MethodTable):

    @impl(NewFood)
    def new_food(
        self,
        interp: FeeAnalysis,
        frame: Frame[Item],
        stmt: NewFood,
    ):
        return (ItemFood(type=stmt.type),)

    @impl(Cook)
    def cook(
        self,
        interp: FeeAnalysis,
        frame: Frame[Item],
        stmt: Cook,
    ):
        # food depends on the food type to have different charge:

        food = frame.get_typed(stmt.target, ItemFood)
        serving_count: AtLeastXItem | ConstIntItem = frame.get(stmt.amount)

        out = ItemServing(count=serving_count, type=food.type)

        return (out,)

    @impl(Nap)
    def nap(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[Item],
        stmt: Nap,
    ):
        interp.nap_count += 1
        return ()

```

## Putting Everything Together

```python
fee_analysis = FeeAnalysis(main2.dialects)
results, expect = fee_analysis.run_analysis(main2, args=(AtLeastXItem(data=10),))
print(results)
print(fee_analysis.nap_count)
```
