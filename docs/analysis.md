!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# Analysis

Kirin provides a framework for performing dataflow analysis on the IR. This is done via

- IR walking
- Abstract interpretation

## Lattice

A lattice is a set of values that are partially ordered. In Kirin IR, a lattice is a subclass of the [`Lattice`][kirin.lattice.abc.Lattice] ABC class. A lattice can be used to represent the result of a statement that has multiple possible results.
