## Beer price/fee analysis

In this section we will discuss on how to perform analysis of a kirin program. We will again use our `beer` dialect example.

### Goal

Let's Consider the following program
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

We would like to implement an forward dataflow analysis that walk through the program, and collect the price information of each statements.

### Define Lattice
One of the important concept related to doing static analysis is the *Lattice* (See [Wiki:Lattice](https://en.wikipedia.org/wiki/Lattice_(order)) and [Lecture Note On Static Analysis](https://studwww.itu.dk/~brabrand/static.pdf) for further details)
A Lattice defines the partial order of the lattice element. An simple example is the type lattice.

Let's now defines our `Item` lattice for the price analysis.

First, a lattice always has top and bottom elements. In type lattice, the top element is `Any` and bottom element is `None`.


Here, we define `AnyItem` as top and `NoItem` as bottom. In kirin, we can simply inherit the `BoundedLattice` from `kirin.lattice`. Kirin also provide some simple mixin with default implementation of the API such as `is_subseteq`, `join` and `meet` so you don't have to re-implement them.

```python
from kirin.lattice import (
    SingletonMeta,
    BoundedLattice,
    IsSubsetEqMixin,
    SimpleJoinMixin,
    SimpleMeetMixin,
)

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
    we can use the SingletonMeta to make it a singleton by inherit the metaclass

    """

    def is_subseteq(self, other: Item) -> bool:
        return True


@final
@dataclass
class AnyItem(Item, metaclass=SingletonMeta):
    """The top of the lattice.

    Since the element is the same without any field,
    we can use the SingletonMeta to make it a singleton by inherit the metaclass

    """

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AnyItem)

```

1. Notice that since `NotItem` and `AnyItem` does not have any properties, we can mark them as singleton to remove duplication copy of instances exist by inheriting `SingletonMeta` metaclass

Next there are a few more lattice elements we want to define:

```python
@final
@dataclass
class ItemPints(Item): # (1)!
    count: Item
    brand: str

    def is_subseteq(self, other: Item) -> bool:
        return (
            isinstance(other, ItemPints)
            and self.count == other.count
            and self.brand == other.brand
        )

@final
@dataclass
class AtLeastXItem(Item): # (2)!
    data: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, AtLeastXItem) and self.data == other.data


@final
@dataclass
class ConstIntItem(Item):
    data: int

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, ConstIntItem) and self.data == other.data


@final
@dataclass
class ItemBeer(Item):
    brand: str

    def is_subseteq(self, other: Item) -> bool:
        return isinstance(other, ItemBeer) and self.brand == other.brand


```

1. `ItemPints` which contain information of the beer brand of `Pints`, as well as the count
2. `AtLeastXItem` which contain information of a constant type result value is a number that is least `x`. The `data` contain the lower-bound
3. `ConstIntItem` which contain concrete number.
4. `ItemBeer` which contain information of `Beer`.


### Custom Forward Data Flow Analysis

Now we have defined our lattice. Let's move on to see how we can write an analysis pass, and get the analysis results.

In kirin, the analysis pass is implemented with `AbstractInterpreter` (inspired by Julia). Kirin provides an simple forward dataflow analysis `Forward`. So we will use that.

Here our analysis want to do two things.

1. Get all the analysis results as dictionary of SSAVAlue to lattice element.
2. Count how many time one puke.

```python
@dataclass
class FeeAnalysis(Forward[latt.Item]): # (1)!
    keys = ["beer.fee"] # (2)!
    lattice = latt.Item
    puke_count: int = field(init=False)

    def initialize(self): # (3)!
        """Initialize the analysis pass.

        The method is called before the analysis pass starts.

        Note:
            1. Here one is *required* to call the super().initialize() to initialize the analysis pass,
            which clear all the previous analysis results and symbol tables.
            2. Any additional initialization that belongs to the analysis should also be done here.
            For example, in this case, we initialize the puke_count to 0.

        """
        super().initialize()
        self.puke_count = 0
        return self

    def eval_stmt_fallback( # (4)!
        self, frame: ForwardFrame[latt.Item, None], stmt: ir.Statement
    ) -> tuple[latt.Item, ...] | interp.SpecialValue[latt.Item]:
        return ()

    def run_method(self, method: ir.Method, args: tuple[latt.Item, ...]) -> latt.Item: # (5)!
        return self.run_callable(method.code, (self.lattice.bottom(),) + args)

```

1. Interit from `Forward` with our customize lattice `Item`.
2. The keys for the MethodTable. Remember that in kirin all the implmentation methods of a interpreter is implmeneted and registered to a `MethodTable`.
3. `AbstractInterpreter` has a abstract method `initialize` which will be called everytime `run()` is called. We can overload this to reset the variable, so we can re-use this class instance.
4. This method implement the *fallback* when interprete a statement that does not have implmenetation found in the method table. Here, we just return an empty tuple because we know all the statements that has return value will be implemented, so only statements without return value will be fallbacked.
5. This method defines and customize how to run the `ir.Method`.

Click the + logo to see more details.

Now we want to implement how the statement gets run. This is the same as what we described when we mentioned the concrete interpreter.

Note that each dialect can have multiple registered `MethodTable`, distinguished by `key`. The interpreter use `key` to find corresponding `MethodTable`s for each dialect in a dialect group.

Here, we use `key="beer.fee"`

First we need to implement for `Constant` statement in `py.constant` dialect. If its `int`, we return `ConstIntItem` lattice element. If its `Beer`, we return `ItemBeer`.

```python
@py.constant.dialect.register(key="beer.fee")
class PyConstMethodTable(interp.MethodTable):

    @interp.impl(py.constant.Constant)
    def const(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: py.constant.Constant,
    ):
        if isinstance(stmt.value, int):
            return (latt.ConstIntItem(data=stmt.value),)
        elif isinstance(stmt.value, Beer):
            return (latt.ItemBeer(brand=stmt.value.brand),)

        else:
            raise exceptions.InterpreterError(
                f"illegal constant type {type(stmt.value)}"
            )
```


Next, since we allow `add` in the program, we also need to let abstract interpreter know how to interprete `binop.Add` statement, which is in `py.binop` dialect.
```python
@binop.dialect.register(key="beer.fee")
class PyBinOpMethodTable(interp.MethodTable):

    @interp.impl(binop.Add)
    def add(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: binop.Add,
    ):
        left = frame.get(stmt.lhs)
        right = frame.get(stmt.rhs)

        if isinstance(left, latt.AtLeastXItem) or isinstance(right, latt.AtLeastXItem):
            out = latt.AtLeastXItem(data=left.data + right.data)
        else:
            out = latt.ConstIntItem(data=left.data + right.data)

        return (out,)
```

Finally, we need implementation for our beer dialect Statements.
```python
@dialect.register(key="beer.fee")
class BeerMethodTable(interp.MethodTable):

    @interp.impl(NewBeer)
    def new_beer(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: NewBeer,
    ):
        return (latt.ItemBeer(brand=stmt.brand),)

    @interp.impl(Pour)
    def pour(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: Pour,
    ):
        # Drink depends on the beer type to have different charge:

        beer: latt.ItemBeer = frame.get(stmt.beverage)
        pint_count: latt.AtLeastXItem | latt.ConstIntItem = frame.get(stmt.amount)

        out = latt.ItemPints(count=pint_count, brand=beer.brand)

        return (out,)

    @interp.impl(Puke)
    def puke(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: Puke,
    ):
        interp.puke_count += 1
        return ()

```

## Put it together:

```python
fee_analysis = FeeAnalysis(main2.dialects)
results, expect = fee_analysis.run_analysis(main2, args=(AtLeastXItem(data=10),))
print(results)
print(fee_analysis.puke_count)
```
