---
date: 2024-11-11
authors:
    - rogerluo
---

# Kirin's Mission

The field of scientific software has seen great development in the past decade. Scientists have been building more and more sophisticated simulators (quantum circuit simulators, differential equation solvers,  differentiable solvers), and many different human controlable physical systems (neutral atoms, trapped ions, superconducting qubits, optical lattices, etc) that may be used for multiple purposes.

These impressive progress urges the need for a more sophisticated way to interact with these simulators and physical systems. Yes, it's not just about new hardware, computation, afterall, is about how to compile a problem description to human-controlable physical systems. In software community, a domain-specific language (DSL) and a corresponding compiler is the way to go. However, building a DSL and a compiler is a non-trivial task, and it is not something that scientists are familiar with.

This is why we are building Kirin - we want to provide a **set of tools for scientists to build their own DSLs and making their DSLs composable with each other**. This article is to explain the mission of Kirin, and why we are building it.

## The fragmentation of scientific software

I cannot talk about all scientific software, but software for quantum many-body system are already quite fragmented. For example, there are many different ways to represent sets of quantum operators system mathematically, such as tensor networks, quantum circuits, ZX diagrams[^3], operator table[^4], symbolic expressions, Quon[^5], etc. Each of these representations may result in a few different DSLs due to different design tradeoffs. In the lab, experimentalists also have different languages, such as pulse sequences, movings, QASM, QIR, etc. Almost every university lab has their own representation of the device, depsite of the sanity (e.g see last section). People have often been thinking if we can create a unified representation for all these problems, but it is not easy. Instead, the dream is: **if we can make representations composable, we can build a unified representation by composing different representations from the experts.**

We, at QuEra Computing Inc., naturally falls into this fragmentation problem, and thus have many duplicated representations in the past. Then the question is, how can we improve composability? There are already an answer in the software engineering community, which is the Multi-Level Intermediate Representation (MLIR). I like MLIR, but we are not using it. I will explain why.

[^3]: ZX diagrams can be seen as tensor networks too, but the implementation is usually different data structure.
[^4]: Aaronson's stabilizer table, stochastic series expansion's operator table, etc.
[^5]: Quon is a 3D representation of quantum operators.

## Why didn't use X?

This is a common question that one may ask when they see a new project. Here are some reasons why we are not using previous work.

### Why not use MLIR?

We have been trying to adopt MLIR since 2021, and we found that MLIR is a great project, but it is not easy to use for scientists. The scope of compilation problems that MLIR is trying to solve is too broad, and thus the complexity of the project is inevitably high. More specifically, we do not care about:

- low-level semantics
- traditional hardware targets
- high-quality native binary
- roundtrip-able IR text format
- speed of the compiler or the generated code [^1]

[^1]: this does not mean the program execution has to be very slow, e.g we can generate a different set of high-level IR that can be interpreted quickly (via optimized instruction implementation, etc.).

We do care about:

- high-level semantics
- new optimization techniques for the scientific problems
- interpretation and JIT compilation
- a frontend that is easy to use for scientists

In the quantum computing community, we have seen a lot of projects that are trying to build a quantum DSL and a compiler using MLIR. However, everyone will build

- their own set of dialects, including control-flow dialects
- Python frontend for the DSL
- another infrastructure to write IR passes in Python

This is mainly because how to compile such program is still under research. Thus we expect normal scientists (e.g theoretical/experimental physicists) to write compiler passes. And it is impossible for them to learn MLIR within one week. As a result, MLIR was never adopted by broader quantum community.

A few existing compilers focus on building an eDSL in Python then JIT compile the kernel to native binary. However, at least in our case, we have to decompile such native binary back to a JSON in order to run the program on a Python-based control system. It turns out what is really needed for a lot of scientists is:

- a way to write a high-level eDSL in Python **(model of the problem)**
- then run some rewrites **(problem simplification/compilation)**
- run it with an interpreter **(model execution)**
- optionally compile it to another IR then run it with another interpreter. **(simulation)**

This seems to be the case not only for quantum physicists but other scientists such as the original machine-learning engineer's use case in [supporting interpreter framework in MLIR](https://discourse.llvm.org/t/rfc-mlir-interpreter-framework/63567). To support such use case, because MLIR needs to consider a lot more complicated senarios, it will take a lot of thinking to do it right.

Big entities with sufficient bandwidth can afford doing this by hiring a lot of engineers with professional compiler engineering background, but for small companies and individual researchers, this is not feasible. We see the issue is due to the complexity of a generic problem, and the lack of involvement of scientists in the design of a compiler infrastructure. We believe that the solution is to build a more user-friendly compiler infrastructure by scientists for scientists. This is possible by limiting the program to

- only kernel functions (program size are small)
- only existing langauge (e.g Python) frontend (so that we can focus on designing composable lowering framework)
- focus on interpretation (or abstract interpretation), turn every compilation into different interpretation steps
- focus on Python-like type system

### Why not use xDSL?

Ok, after we have decided not to use MLIR, we were excited to see this project called [xDSL](https://github.com/xdslproject/xdsl) around late 2023 and tried to play with it. However, we found that the project is still sophisticated for scientists to use. Mostly because the project is built by professional compiler engineers with a great familarity of MLIR. This is an advantage for the project to be able to support a wide range of problems and being able to interop with MLIR. The project is great, if you are interested in a friendlier version of MLIR in Python, you should definitely check it out. However, as a result, the priority of the project is not to build something scientists can get started with in a few days. What was missing from our wishlist is:

- composable Python lowering (e.g Python syntax claims from different dialects)
- different ways of interpretation (e.g abstract interpretation)
- Python type system instead of MLIR type attributes
- dialects that supports Python semantics instead of MLIR dialects

while maybe eventually this would be in xDSL but more realistically, because the focus is quite different and xDSL was also new, the priorities are different.

### Why not Julia compiler plugin?

For those who are familiar with my past work, I have been working with Julia since 2015. This is a great language
for scientific computing, and scientists with performance needs definitely love it. And in fact, the idea of
building some infrastructure to help scientists to build their own DSLs and compilers was initially from the
Julia community because of automatic differentiation and sparsity optimization (check the [CompilerPlugin org](https://github.com/JuliaCompilerPlugins) and [Symbolics](https://github.com/JuliaSymbolics/Symbolics.jl)+[ModelingToolkit](https://github.com/SciML/ModelingToolkit.jl)). To enable automatic differentiation, we also end up building a small DSL in [Yao](https://github.com/QuantumBFS/Yao.jl) to represent quantum circuits. In fact, to support automatic differentiation well, a lot of packages end up building their own DSLs and compilers.

However, when I start moving towards a compiler for quantum circuit ([YaoCompiler](https://github.com/QuantumBFS/YaoCompiler.jl)), the instability of the Julia compiler internal API quickly becomes a problem. On the other hand, the Julia SSA IR was not designed for custom DSLs, e.g it does not have the region semantics to represent hierarchical IRs, and it does not have the abstraction of dialects to allow composing different DSLs. While we have seen some promising progress in [MLIR.jl](https://github.com/JuliaLabs/MLIR.jl), the project is still in its early stage.

On the other hand, I do get jelous of how Julia compiler solve the dynamic language compilation problem, thus I learnt a lot from the Julia compiler and the Julia compiler plugin. This results in the design of the Kirin type system, and the abstract interpretation framework. [^2]

[^2]: Kirin does not support multiple dispatch at the moment, this may be added in the future. I'm hesitating to add multiple dispatch because this turns out less useful for kernels because they can be embeded in host language abstraction most of the time.

Python has been a hard requirement for us in building production software. But I reserve the right to use Julia to rewrite certain components of Kirin in the future.

### Why not use JAXPR?

JAX is a great project for automatic differentiation and scientific computing. I have been using it extensively for a few projects. While it is true that JAXPR covers a wide range of scientific computing tasks, it is not designed for more complicated tasks like quantum compilation, especially when you work at levels lower than quantum circuits. The limitation of JAX is intentionally designed to allow only tracing-based compilation, thus it does not support native Python control flows, and may not have clear function barriers in the traced program unless you use `jit` to separate them. The runtime value is restricted to JAX arrays, etc. These tradeoffs may not always be suitable for building a quantum program. The JAX compiler is designed for differentiable/machine-learning linear algebra programs afterall.

## How would Kirin be different?

While Kirin draws inspiration from MLIR, xDSL, Julia compiler plugin, and JAXPR, we aim to build a more user-friendly compiler infrastructure for scientists to solve their specific problems. There are nothing fundamentally new in theory, but the combination is new. Here are some key differences:

**Composable Python Lowering**, in our food-lang example, the kernel decorator `@food` is just
a `DialectGroup` object that contains the `Dialect` objects you specified to include for the frontend. The Python syntax just maginally works! This is because Kirin features a composable lowering system that allows you to claim Python syntax from each separate dialect. When combining the dialects together, Kirin will be able to compile each Python syntax to the corresponding IR nodes, e.g

- `func` dialect claims function related syntax: the `ast.FunctionDef`, nested `ast.FunctionDef` (as closures), `ast.Call`, `ast.Return`, etc.
- `cf` (unstructured control flow) dialect claims control flow related syntax: `ast.If`, `assert`.
- `math` dialect claims python builtin math functions
- `py.stmts` claims miscellaneous other Python statements, e.g assignment, comparison, slice, range, etc.

You can define your own lowering rules for new dialects, and compose with other dialects to build your own frontend.

**Abstract Interpretation**, inspired by Julia abstract interpreter, Kirin features a simple abstract interpretation framework that allows you to define how to interpret your own dialects on a given [lattice](https://en.wikipedia.org/wiki/Lattice_(order)). This allows Kirin to perform analysis such as constant propagation and type inference on your program. For example, like Julia, Kirin can perform constant propagation interprocedurally, and infer the types of your program, and this is composable with new dialects.

**Python Type System**, based on the abstract interpretation framework, the `py.types` dialect defines a Python type system as Kirin type attributes. This allows dynamic and generic definitions of dialects. This becomes quite useful defining high-level Python-like semantics, such as a statement taking a list of integers, or a statement taking a callable object. Taking the example from tests, you can just write the following in your frontend and let Kirin figure out the types

```python
@basic
def foo(x: int):
    if x > 1:
        return x + 1
    else:
        return x - 1.0


@basic(typeinfer=True)
def main(x: int):
    return foo(x)


@basic(typeinfer=True)
def moo(x):
    return foo(x)
```

prints the following

```python
moo.print()
```

![typeinfer-basic](typeinfer-basic.png)

_while `py.types` supports generics, the type inference is still work-in-progress on generics._

**IR Declaration Decorators**, like `xDSL`, Kirin also provides the `statement` decorator to declare a new IR statement. However, the difference is Kirin focus on reducing extra terminologies and concepts that are not familiar to scientists with the cost of being more entangled with Python. The `statement` decorator is designed to be `@dataclass`-like, which is what scientists are familiar with. This is the only decorator one needs to know. Fields declared with native Python types are automatically converted to Kirin attributes via the Python type system.

Taking an example from xDSL, you would define `Func` as

```python
@irdl_op_definition
class FuncOp(IRDLOperation):
    name = "func.func"

    body = region_def()
    sym_name = prop_def(StringAttr)
    function_type = prop_def(FunctionType)
    sym_visibility = opt_prop_def(StringAttr)
    arg_attrs = opt_prop_def(ArrayAttr[DictionaryAttr])
    res_attrs = opt_prop_def(ArrayAttr[DictionaryAttr])

    traits = traits_def(
        IsolatedFromAbove(), SymbolOpInterface(), FuncOpCallableInterface()
    )
```

and you will need to define `__init__` etc. in the following.
In Kirin, you would define `Func` as

```python
@statement(dialect=dialect)
class Function(Statement):
    name = "func"
    traits = frozenset(
        {
            IsolatedFromAbove(),
            SymbolOpInterface(),
            HasSignature(),
            FuncOpCallableInterface(),
            SSACFGRegion(),
        }
    )
    sym_name: str = info.attribute(property=True)
    signature: Signature = info.attribute()
    body: Region = info.region(multi=True)
```

which is a more `@dataclass`-like definition, where fields with types are instance fields, and fields without types are (class) properties. The `@statement` also generates the corresponding `__init__` etc. just like a `@dataclass` (and your linter understands it!).

## Conclusion

When we start thinking about these scientific problems seriously, there seems not many in the software engineering community that would worry about the scientists. The scientists are living with a lot of workaround solutions, including but not limited to "define the language as a string, where each character represents an operation/statement, then let a human perform the control-flow" (the man in-the-loop). The compiler community are often excited about programming language for everything. However, for scientific community, especially nature science, there seems to be a big gap between the compiler community and the natural-science community (e.g a physicist like me) to make this come true.

Kirin in its current form, may not be perfect (likely). However, we hope we can share this same vision for the scientific software community. So that Kirin, as a vision, can bridge this gap and help scientists to push the boundary of science with scientific software.
