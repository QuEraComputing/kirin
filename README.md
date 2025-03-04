# KIRIN

[![CI](https://github.com/QuEraComputing/kirin/actions/workflows/ci.yml/badge.svg)](https://github.com/QuEraComputing/kirin/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/QuEraComputing/kirin/graph/badge.svg?token=lkUZ9DTqy4)](https://codecov.io/gh/QuEraComputing/kirin)
[![Supported Python versions](https://img.shields.io/pypi/pyversions/kirin-toolchain.svg?color=%2334D058)](https://pypi.org/project/kirin-toolchain)

*K*ernel *I*ntermediate *R*epresentation *IN*frastructure

> [!IMPORTANT]
>
> This project is in the early stage of development. API and features are subject to change.
> If you are concerned about the stability of the APIs, consider pinning the version of Kirin in your project.

## Installation

### Install via `uv` (Recommended)

```py
uv add kirin-toolchain
```

### Install via pip

```bash
pip install kirin-toolchain
```

## Documentation

The documentation is available at [https://queracomputing.github.io/kirin/latest/](https://queracomputing.github.io/kirin/latest/). We are at an early stage of completing the documentation with more details and examples, so comments and contributions are most welcome!

## Community

- **Slack**: join our [Slack](https://join.slack.com/t/kirin-1lj5658/shared_invite/zt-30qhwg83r-fTUdXF9w47nTiNFgO18X4w).
- **GitHub Discussions**: discussion board for questions, feature requests, and more. [GitHub Discussions](https://github.com/QuEraComputing/kirin/discussions).

## Projects using Kirin

### Quantum Computing

We are actively using Kirin at QuEra Computing. Here are some open-source eDSLs for quantum computing that we have developed using Kirin:

- [bloqade.qasm2](https://github.com/QuEraComputing/bloqade/tree/main/src/bloqade/qasm2): This is an eDSL for quantum computing that uses Kirin to define an eDSL for the Quantum Assembly Language (QASM) 2.0. It demonstrates how to create multiple dialects using Kirin, run custom analysis and rewrites, and generate code from the dialects (back to QASM 2.0 in this case).
- [bloqade.stim](https://github.com/QuEraComputing/bloqade/tree/main/src/bloqade/stim): This is an eDSL for quantum computing that uses Kirin to define an eDSL for the [STIM](https://github.com/quantumlib/Stim/) language. It demonstrates how to create multiple dialects using Kirin, run custom analysis and rewrites, and generate code from the dialects (back to Stim in this case).
- [bloqade.qBraid](https://github.com/QuEraComputing/bloqade/blob/main/src/bloqade/qbraid/lowering.py): This example demonstrates how to lower from an existing representation into the Kirin IR by using the visitor pattern.

## License

Apache License 2.0 with LLVM Exceptions
