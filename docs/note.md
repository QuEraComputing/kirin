We are iterating the design of interpreter framework in kirin-interpreter-* crates (e.g kirin-interpreter-5). Review the existing implementations and designs in the previous interpreter crates, critize them and design a new interpreter framework that supports the following requirements:

# Requirements

## Lift/Project Algebra
The lift/project algebra connects the total and dialect-specific objects. Given a total object type (e.g the total effect type), and a dialect type that is part of the total dialect sum, we should be able to project the total object into the dialect-specific object.

Similarly, given a dialect-specific object and the total dialect sum, we should be able to lift the dialect-specific object into the total object.

## Supporting different interpreters

We should support both concrete interpreters (e.g. a standard interpreter that executes code directly) and abstract interpreters (e.g. an analysis interpreter that tracks value sets instead of concrete values). The lift/project algebra should work uniformly across both kinds of interpreters.

## Supporting both stage-specific and multi-stage interpretation

For both concrete and abstract interpreters, we should support both stage-specific interpretation (where each stage is executed separately, and the interpretation is specialized on the stage's language) and multi-stage interpretation (dynamic via stage enum, where stages can invoke each other directly).

## Semantics, typing rules as code

The operational, denotional semantics, typing rules can be defined as code in the interpreter framework. This means one can verify the semantics and typing rules by running tests with the interpreter (either via concrete or abstract interpreter).

## Dialect specific stays dialect local

Dialect author should be able to define dialect-specific semantics such as the operational semantics, effects, errors and cursors locally in the dialect crate, without needing to modify the interpreter or other dialects. This means that the dialect-specific semantics should not leak into the interpreter or other dialects.

# Testing

We must test our interpreter framework design by implementing the corresponding traits for all dialect crates as a new submodule `interpreter<iteration>`(e.g `interpreter8`), and write tests to verify the semantics and typing rules of the dialects. We should also test the composition of dialects and interpreters, and verify that the lift/project algebra works correctly across different interpreters and stages.

Finally we should implement the interpreter for toy-lang example and verify it works cross stages and with both concrete and abstract interpretation. The toy-lang example tests should cover all the test cases mentioned in the previous interpreter crates, and also test the new features.

# Priority

1. Extensibility - the framework should be extensible for new interpreter types.
2. Algebraic elegance - the lift/project algebra should be elegant and consistent across different interpreters and stages. If you design new algebra for other aspects of the interpreter, it should also be algebraically elegant and consistent.
3. Correctness - do not workaround the limitations of the Rust type system by using unsafe code, `'static` lifetimes, `Arc`, or other escape hatches. The design should be correct by construction, and the Rust type system should be able to verify it without needing unsafe code or other escape hatches.
4. DRY - avoid code duplication as much as possible, code duplication is a sign of wrong abstraction, you should think about how to abstract the common patterns and avoid code duplication. However, do not sacrifice correctness or extensibility for DRY, if you need to duplicate some code to make the design correct or extensible, it's fine.

# Design Rationale

1. dialect authors are exposed to minimal trait surface for defining semantics they need, no boilderplate for composition, and no need to understand the internal mechanics of the interpreter.
2. the local semantic effect can be done via mutating within Interpretable trait, the global, multi-step effect should be done via returning Effect from Interpretable trait, and the interpreter is responsible for routing the effect to the dialect machine and mutating the machine state accordingly.
3. do not worry about derive, some of the traits especially for those built on top of composition has natural derive patterns, we can implement the derive macro later, for now we can just write the impls manually, and verify the design works correctly. But leave a comment in the code as "TODO: replace this with derive macro" to indicate that this is a temporary implementation and we can replace it with derive macro later.
4. Do not use Box, Arc, Rc, or other heap allocation for the core design, we should give developer the freedom to choose how they compose, instead of forcing them to use things like `Box<dyn Execute>` for composition. Use generics and traits to achieve composition, and assume `enum` for composition (e.g composing effects, cursor or dialects), the developer can choose to use `Box` or `Arc` if they want to, but the core design should not enforce it. This is also important for the lift/project algebra, we want to be able to lift/project without needing heap allocation, and we want to be able to do it in a zero-cost way.

# Steps

- Critize the latest design and implementation, identify the pain points and limitations, confirm with the user these are the pain points and limitations they want to address in the new design. If nothing to criticize, then we can just say "the latest design and implementation is good, we can keep it as is and iterate on it in the future if needed".

LOOP FOREVER:
- Design the new interpreter framework that supports the requirements, and write down the design rationale for each design decision.
- Maintain a design principle document `design_principles.md` in the `docs/` folder (create it if not exist), and update it with the design principles and rationale for the new interpreter framework, keep it up to date with the design iterations and changes. This should be latest design principles and rationales, if previous design principles are no longer valid, we can remove them from the document or modify them to reflect the new design principles.
- Implement the new interpreter framework in a new crate `kirin-interpreter-<iteration>`, and implement the corresponding traits for all dialect crates as a new submodule `interpreter<iteration>`.
- Write tests to verify the semantics and typing rules of the dialects, and also test the composition of dialects and interpreters, and verify that the lift/project algebra works correctly across different interpreters and stages.
- Implement the interpreter for toy-lang example and verify it works cross stages and with both concrete and abstract interpretation.
- Commit the new design and implementation (do not commit `docs/`)
