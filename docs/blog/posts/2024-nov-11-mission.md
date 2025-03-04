---
date: 2025-02-28
authors:
    - rogerluo
    - kaihsin
    - weinbe58
    - johnzl-777
    - jwurtz
    - yuval
    - dean
    - shengtao
    - takuya
---

# Introducing Kirin, a new open-source software development tool for fault-tolerant quantum computing

Today, we are excited to introduce **Kirin (Kernel Intermediate Representation INfrastructure)**, a new Python-based compiler infrastructure designed to simplify how scientists and engineers create embedded domain-specific languages (eDSLs)—especially in the space of quantum computing. We are introducing this alongside the next generation of the Bloqade SDK (v0.16) — the neutral atom quantum computing software development kit.

!!! note
    if you are unfamiliar with eDSLs, see the [appendix](#appendix-the-growing-need-for-edsls-in-quantum-science) for a more detailed description of the motivation and use of eDSLs.

[Bloqade](https://bloqade.quera.com) will serve as a hub for eDSLs targeting neutral-atom quantum computers. Kirin introduces a powerful system of dialects and a robust Intermediate Representation (IR) that together address the unique challenges in quantum programming, scientific simulation, and hardware control. In this inaugural release (v0.21), we introduce a **QASM2 eDSL** and its neutral-atom extensions, powered by Kirin.

Together, these open-source tools are designed to empower scientists and developers to push the boundaries of quantum computing and scientific research.

## What is Kirin?

Kirin is a new compiler infrastructure designed to simplify the development of **embedded domain-specific languages (eDSLs)**, particularly for quantum computing and other scientific domains. In quantum computing, fragmented representations of quantum objects—such as circuits, pulses, noise models, control flows, and tensor networks—create significant challenges in integrating them into a cohesive framework. Kirin addresses these challenges by providing tools to **unify and compose** these diverse representations and their corresponding compilation processes.

## Kirin’s mission

Kirin empowers scientists to build tailored embedded domain-specific languages (eDSLs) by adhering to three core principles:

1. **Scientists First**

    Kirin prioritizes enabling researchers to create compilers for scientific challenges. The toolchain is designed *by* and *for* domain experts, ensuring practicality and alignment with real-world research needs.

2. **Focused Scope**

    Unlike generic compiler frameworks, Kirin deliberately narrows its focus to scientific applications. It specializes in high-level, structurally oriented eDSLs—optimized for concise, kernel-style functions that form the backbone of computational workflows.

3. **Composability as a Foundation**

    Science thrives on interdisciplinary collaboration. Kirin treats composability—the modular integration of systems and components—as a first-class design principle. This ensures eDSLs and their compilers can seamlessly interact, mirroring the interconnected nature of scientific domains.


### What are Kernel Functions?

A kernel function is a piece of code that runs on specialized hardware—like a GPU, a cluster, or a quantum computer—instead of going through normal Python interpretation. While this concept may be unfamiliar in the field of quantum computing, it's common in machine learning frameworks like JAX ([jax jit](https://docs.jax.dev/en/latest/jit-compilation.html), [pallas](https://docs.jax.dev/en/latest/pallas/index.html)), [PyTorch](https://pytorch.org/docs/stable/jit.html) or [CUDA.jl](https://cuda.juliagpu.org/stable/development/kernel/), where the `@jit` decorator marks Python functions as kernels for compilation and optimization.

### **Example: Extending QASM2 with Kirin**

Suppose you have an existing quantum circuit language (like OpenQASM 2) and you would like to add **recursion and loops** — a concept that your original circuit language doesn’t have. Let’s see how this works.

The baseline `qasm2.main` decorator adheres strictly to QASM2 specifications:

```python
from bloqade import qasm2

@qasm2.main
def main():
    qreg = qasm2.qreg(2) # quantum register
    creg = qasm2.creg(2) # classical register
    qasm2.h(qreg[0]) # Hadamard gate
    qasm2.measure(qreg[0], creg[0]) # measure
    if creg[0] == 0: # simple conditional gate
       qasm2.x(qreg[0])
```

You may quickly find it restrictive when writing more complicated examples due to the lack of some common Python language features such as recursions and loops. To address this, we provide a `qasm2.extended` decorator that introduces some common Python language features as well as some special gates for the neutral atom platform. For instance, see the following Quantum Fourier Transform (QFT) example written with recursion:

```python
import math
from bloqade import qasm2

@qasm2.extended
def qft(qreg: qasm2.QReg, n: int, k: int):
    if k == n:
        return qreg

    qasm2.h(qreg[k])
    for i in range(k + 1, n):
        qasm2.cu1(qreg[i], qreg[k], 2 * math.pi / 2**i)
    qft(qreg, n, k + 1)  # recursion
    return qreg
```

Below, you can see what this immediately compiles to. While we aren’t going to dive too deeply into what the output means, the interested readers can learn more about it in [Kirin’s documentation](https://queracomputing.github.io/kirin/latest/101/).

<figure markdown="span">
  ![QFT IR](qft-code.png){ width="800" }
  <figcaption>Compilation result of Bloqade’s extended qasm2 kernel of a QFT program.</figcaption>
</figure>

At a high-level, this is an Intermediate Representation (IR) that the compiler emits. By representing a human-readable program in a machine-friendly IR, it allows us to further compile the program into other lower-level representations such as atom moves and pulses for neutral atom quantum computers.

## Dialects - multiple eDSL made simple

At QuEra, we can design and compile many different eDSLs to enable fast iterations on our machine architecture. This begs the question: how do we easily compose and maintain them?

The answer to the problem in Kirin is **dialects** — smaller, highly composable units of languages. While the actual implementation of the `@qasm2.extended` eDSL involves several dialects, in general, it introduces the QASM2 unitary operator into Python, so equivalently you can just write

```python
from kirin.prelude import basic
from bloqade import qasm2

@basic.add(qasm2.uop)
def qft(qreg: qasm2.QReg, n: int): ... # same as previous
```

where the `basic` dialect provides the basic Python language support (e.g., functions, control flows) and the `qasm2.uop` dialect provides the QASM2 gates. Thus, creating a new eDSL on top of an existing one is very simple:

- step 1: Define the semantics you want to add on top of an existing eDSL as dialects
- step 2: Add them to the existing eDSL via `.add` method

Kirin, the compiler infrastructure, helps you define the compilers for different dialects and easily compose multiple compiler components made for different dialects.

## **Example: Extending an eDSL to support parallel gates with Kirin**

Suppose you have an existing quantum circuit language (in OpenQASM 2) and would like to add **parallel gates**—a concept that your hardware supports but the original language doesn’t. The neutral-atom quantum computers at QuEra are capable of a high degree of parallelism, which calls for a parallel dialect that provides the semantics of parallel gates. For example, you can write a [GHZ state preparation circuit using parallel gates](https://bloqade.quera.com/latest/digital/examples/ghz/) with a shorter circuit depth compared to a typical serial execution. This parallelism is also what makes our device powerful in implementing certain transversal gates that are desirable for quantum error correction (QEC).

To support parallel gates with Kirin, you would:

To support parallel gates with Kirin, you would:

1. **Define a dialect** that expresses parallel operations (for example, `parallel.u` or `parallel.cz`).
2. **Compose** that dialect with your base language dialect(s).
3. **Decorate** your Python function with Kirin’s compiler directive, so it compiles to the new IR.

```python
# a simplified version of single layer circuit in log-depth
# GHZ state preparation with parallel gates
import math
from bloqade import qasm2
from kirin.prelude import basic
from kirin.dialects import ilist

n = 4
n_qubits = int(2**n)

@basic.add(qasm2.dialects.parallel)
def ghz_layer(i_layer: int, qreg: qasm2.QReg):
    step = n_qubits // (2**i_layer)
    def get_qubit(x: int):
        return qreg[x]

    ctrl_qubits = ilist.map(fn=get_qubit, collection=range(0, n_qubits, step))
    targ_qubits = ilist.map(
        fn=get_qubit, collection=range(step // 2, n_qubits, step)
    )
    # Ry(-pi/2)
    qasm2.parallel.u(qargs=targ_qubits, theta=-math.pi / 2, phi=0.0, lam=0.0)
    # CZ gates
    qasm2.parallel.cz(ctrls=ctrl_qubits, qargs=targ_qubits)
    # Ry(pi/2)
    qasm2.parallel.u(qargs=targ_qubits, theta=math.pi / 2, phi=0.0, lam=0.0)
```

Under the hood, Kirin translates this code into its IR, combining Python semantics such as closure (from `basic`) with the parallel gate semantics (from `qasm2.dialects.parallel`). You can then run optimization passes or map the IR to specific device instructions—**all without rewriting your entire stack**. For convenience, we wrap the above extension with some modification in a dialect group  `@qasm2.extended`  demonstrated previously in the Bloqade SDK.

## Outlook and Future Plans

This initial release of Kirin centers on the **infrastructure** — the IR, dialect system, and Python integration — so you can:

1.	**Prototype New eDSLs**: Start defining your own dialects or try out the built-in ones (for quantum gates, parallel operations, etc.).
2.	**Optimize with Custom Passes**: Take advantage of IR transformations to optimize your kernel for speed, resource usage, or hardware constraints.
3.	**Contribute**: Share your domain expertise by creating new dialects or compiler transforms —be it for advanced noise modeling, novel quantum error correction strategies, or new compilation algorithms.

Looking ahead, we plan to release additional dialects and compiler tools that will enable:

1. Programming with lower-level neutral atom machine concepts, including atom movement
2. Hardware-specific optimizations to improve execution fidelity and performance on the neutral-atom platform
3. A fully functional pipeline to access our state-of-the-art neutral-atom hardware

By open-sourcing both our quantum eDSLs (Bloqade) and infrastructure (Kirin), we invite the entire community to join us in advancing quantum computing. Together, we can push the boundaries of what's possible.

## Try it out

- **Download and Install**: Check out Kirin’s [repository on GitHub](https://github.com/QuEraComputing/kirin) and install via `pip`:

```bash
pip install kirin-toolchain
```
- **Documentation and Examples**: Visit our [docs site](https://queracomputing.github.io/kirin/latest/) for a getting-started guide and examples.

## Community

We encourage everyone join the effort to build this infrastructure together

- **Slack** join via [this link](https://join.slack.com/t/kirin-1lj5658/shared_invite/zt-30qhwg83r-fTUdXF9w47nTiNFgO18X4w) to meet and share your ideas about Kirin with the community!
- **GitHub Discussion**: ****Have trouble using Kirin but not sure if it should be a bug report? Or want to announce your package built using Kirin? Or more public discussion or announcement in the community? Try Kirin’s [GitHub Discussion](https://github.com/QuEraComputing/kirin/discussions).
- **Star Kirin on GitHub**: If you like what you see, please give us a star on [GitHub](https://github.com/QuEraComputing/kirin)!

Whether you’re a quantum researcher, a compiler engineer, or just passionate about building next-generation compiler tools, **Kirin** is here to help you design, optimize, and compose advanced computational workflows with ease. We can’t wait to see what you’ll create!

## Appendix: The Growing Need for eDSLs in Quantum Science

Over the past decade, the fields of quantum computing and scientific research have seen remarkable progress, both in **software** and **hardware**. Scientists have developed increasingly sophisticated simulators—such as quantum circuit simulators, differential equation solvers, and differentiable solvers—as well as a variety of highly controllable quantum systems, including neutral atoms, trapped ions, superconducting qubits, and atoms in optical lattices. These systems are now being used for a wide range of applications.

This rapid progress underscores the need for more advanced tools to effectively interact with these simulators and physical systems. To address this, scientists have created numerous **domain-specific languages (DSLs)** and their corresponding compilers. Examples include:

- **Quantum Circuit Languages**: Quipper, ProjectQ, Q#, Cirq, OpenQASM 2 & 3, PennyLane, Stim
- **Tensor Network Libraries**: ITensor, OMEinsum, QUIMB, Tenet

At its core, computations translate problem descriptions into signals controlling physical systems, making every computation fundamentally a compilation challenge. Building domain-specific languages (DSLs) and compilers is complex, typically beyond most scientists' expertise. Specialized domains like circuit simplification, hardware engineering, atomic physics, and quantum error correction require deep integration into software. Relying on a small compiler engineering team to develop these DSLs is inherently impractical at the current stage of quantum.

We are scientists, just like you, striving for scientific breakthroughs. But we’ve found ourselves stuck trying to build complex compilers to program our own machines and simulators. Existing tools are either too complex for scientists to use effectively or not flexible enough to adapt to our specific needs. These challenges are not unique to the neutral-atom community—they exist across the broader quantum computing community.

### How Kirin Fits Into the Bigger Picture

Kirin isn’t just another compiler — it’s a **framework** for building compilers. If you’re working with:

- **Quantum Hardware** like neutral-atom arrays, trapped ions, or superconducting qubits
- **Classical HPC** kernels for domain-specific simulation workflows
- **Hybrid Quantum-Classical** blend quantum and classical workflows treating both as first-class citizens

Kirin’s modular design gives you a foundation to express your domain logic, refine it through compiler passes, and ultimately generate efficient, hardware-friendly instructions. Because Kirin is built by scientists *for* scientists, it emphasizes approachable abstractions and leaves the door open for community-driven innovation.

![puzzle-pieces.png](puzzle-pieces.png)

## Acknowledgement

While the mission and audience may be very different, Kirin has been deeply inspired by a few projects:

- [MLIR](https://mlir.llvm.org/), the concept of dialects and the way it is designed.
- [xDSL](https://github.com/xdslproject/xdsl), about how IR data structure & interpreter should be designed in Python.
- [Julia](https://julialang.org/), abstract interpretation, and certain design choices for scientific community.
- [JAX](https://jax.readthedocs.io/en/latest/) and [numba](https://numba.pydata.org/), the frontend syntax and the way it is designed.
- [Symbolics.jl](https://github.com/JuliaSymbolics/Symbolics.jl) and its predecessors, the design of rule-based rewriter.

Part of the work is also inspired in previous collaboration in YaoCompiler, thus we would like to thank [Valentin Churavy](https://github.com/vchuravy) and [William Moses](https://github.com/wsmoses) for early discussions around the compiler plugin topic. We thank early support of the YaoCompiler project from [Unitary Foundation](https://unitary.foundation/).
