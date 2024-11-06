# Kirin

Kirin is the *K*ernel *I*ntermediate *R*epresentation *In*frastructure. It is a compiler
infrastructure for building compilers for domain-specific languages (DSLs) that target
scientific computing kernels.

## Features

- MLIR-like dialects as composable python packages
- Generated Python frontend for your DSLs
- Pythonic API for building compiler passes
- Julia-like abstract interpretation framework
- Builtin support for interpretation
- Builtin support Python type system and type inference
- Type hinted via modern Python type hints

## Kirin's mission

Compiler toolchain for scientists. Scientists are building domain-specific languages (DSLs) for
scientific purposes. These DSLs are often high-level, and their instructions are usually slower
than the low-level instructions and thus result in smaller programs. The performance does not need
to be as good as a native program, but scientists want good interactivity and fast prototyping.
Most importantly, scientists usually just want to write Python as their frontend.
