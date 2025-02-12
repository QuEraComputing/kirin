## Beer price analysis

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
