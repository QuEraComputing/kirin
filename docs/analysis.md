!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# Analysis

Kirin provides a set of analysis tools for common analysis tasks and for building new analysis tools.

## Forward Dataflow Analysis

The forward dataflow analysis is a common analysis technique that computes the dataflow information of a program by propagating the information forward through the control flow graph. The forward dataflow analysis is implemented in the [`kirin.analysis.Forward`][kirin.analysis.Forward] class.

The build a forward dataflow analysis, you need to define a lattice. The lattice represents the set of values that can be computed by the analysis.

### Lattice

A lattice is a set of values that are partially ordered. In Kirin IR, a lattice is a subclass of the [`Lattice`][kirin.lattice.abc.Lattice] ABC class. A lattice can be used to represent the result of a statement that has multiple possible results.

The `kirin.lattice` module provides a set of base and mixin classes that can be used to build some common lattices.

Most of the lattices are bounded lattices, which can be implemented by using the [`BoundedLattice`][kirin.lattice.abc.BoundedLattice] abstract class.

Some lattice elements are singleton, which means the lattice class represents a single instance. A metaclass [`SingletonMeta`][kirin.lattice.abc.SingletonMeta] is provided to create singleton lattice class that guarantees only one instance will be created, e.g the following is a trivial lattice named `EmptyLattice`:

```python
class EmptyLattice(BoundedLattice["EmptyLattice"], metaclass=SingletonMeta):
    """Empty lattice."""

    def join(self, other: "EmptyLattice") -> "EmptyLattice":
        return self

    def meet(self, other: "EmptyLattice") -> "EmptyLattice":
        return self

    @classmethod
    def bottom(cls):
        return cls()

    @classmethod
    def top(cls):
        return cls()

    def __hash__(self) -> int:
        return id(self)

    def is_subseteq(self, other: "EmptyLattice") -> bool:
        return True
```

where the lattice is a `BoundedLattice` and it is also a singleton class, which means the class will only have one instance.

### Putting things together

To build a forward dataflow analysis, you need to define a lattice and subclass the `Forward` class. The following is the type inference analysis (simplified) that infers the type of each variable in the program:

```python
class TypeInference(Forward[types.TypeAttribute]):
    keys = ["typeinfer"]
    lattice = types.TypeAttribute
```

where the class `TypeInference` is actually the same as the [interpreter](interp.md) we introduced before, but instead of running on concrete values, it runs on the lattice values and walks through all the control flow branches. The field `keys` is a list of keys that tells the interpreter which method registry to use to run the analysis (similar to the key `"main"` for concrete interpretation).

## Control Flow Graph

The control flow graph (CFG) can be constructed by calling the [`CFG`][kirin.analysis.cfg.CFG] class constructor. The CFG is a directed graph that represents the control flow of the program. Each node in the graph represents a basic block, and each edge represents a control flow transfer between two basic blocks.

An example of using the CFG class to construct a CFG is shown below:

```python
from kirin.analysis import CFG
from kirin.prelude import basic

@basic
def main(x):
    if x > 0:
        y = 1
    else:
        y = 2
    return y


cfg = CFG(main.callable_region)
cfg.print()
```

prints the following directed acyclic graph:

```
Successors:
^0 -> [^1, ^2]
^2 -> [^3]
^3 -> []
^1 -> [^3]

Predecessors:
^1 <- [^0]
^2 <- [^0]
^3 <- [^2, ^1]
```

## API References

::: kirin.analysis.Forward
::: kirin.analysis.cfg.CFG
