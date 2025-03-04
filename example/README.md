# Examples

This folder contains examples of how to use the Kirin library. Each example is a standalone project that can be run independently.

## List of Examples

- `simple.py`: A simple example that demonstrates how to create a simple Kirin dialect group and its kernel.
- `food`: A more sophisticated example but without any domain specifics. It demonstrates how to create a new Kirin dialect and combine it with existing dialects with custom analysis and rewrites.

## Examples outside this folder with more domain-specific contents

### Quantum Computing

- [bloqade.qasm2](https://github.com/QuEraComputing/bloqade/tree/main/src/bloqade/qasm2): This is an eDSL for quantum computing that uses Kirin to define an eDSL for the Quantum Assembly Language (QASM) 2.0. It demonstrates how to create multiple dialects using Kirin, run custom analysis and rewrites, and generate code from the dialects (back to QASM 2.0 in this case).
- [bloqade.stim](https://github.com/QuEraComputing/bloqade/tree/main/src/bloqade/stim): This is an eDSL for quantum computing that uses Kirin to define an eDSL for the [STIM](https://github.com/quantumlib/Stim/) language. It demonstrates how to create multiple dialects using Kirin, run custom analysis and rewrites, and generate code from the dialects (back to Stim in this case).
- [bloqade.qBraid](https://github.com/QuEraComputing/bloqade/blob/main/src/bloqade/qbraid/lowering.py): This example demonstrates how to lower from an existing representation into the Kirin IR by using the visitor pattern.
- [bloqade.analysis](https://github.com/QuEraComputing/bloqade/tree/main/src/bloqade/analysis/): This directory contains examples of how to write custom analysis passes using Kirin for quantum computing.
