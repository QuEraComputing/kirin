use kirin_ir::{
    Block, CompileStage, Function, Product, Region, SSAValue, SpecializedFunction, StagedFunction,
    Symbol,
};

/// The closed forward control algebra a statement produces.
///
/// Atomic statements read operands, write results, and return [`ForwardEffect::Next`].
/// Control statements name successor edges, calls, or returns; the engine owns
/// how each is driven. Structured dialects do **not** get a framework "scope"
/// concept: when a statement needs to run a sub-computation it [pushes a
/// frame](ForwardEffect::Push) it owns (or is handed by the engine), so all
/// structured traversal lives in dialect/engine frames, never in this enum.
///
/// `F` is the engine's total frame type (the same `F` that parameterizes
/// [`ConcreteInterpreter`](crate::ConcreteInterpreter)/
/// [`ForwardAbstractInterpreter`](crate::ForwardAbstractInterpreter)).
/// Ordinary dialect rules never name `F`: they only build the frame-free
/// variants, so `F` is inferred from `I::Effect`. Only a dialect whose operations
/// own structured traversal (e.g. `kirin-scf`'s `scf.if`/`scf.for`) builds
/// [`Push`](ForwardEffect::Push), carrying a frame **it owns**. The framework has
/// no "explore alternatives" effect: a dialect frame that needs to explore
/// several bodies pushes them one at a time and joins their results itself.
pub enum ForwardEffect<V, F> {
    /// Statement done; continue with the next statement.
    Next,
    /// Unconditional transfer to a block in the current region.
    Jump(Edge<V>),
    /// Conditional transfer whose condition is undecided in the value domain.
    Branch(Vec<Edge<V>>),
    /// Invoke a function through the engine's [`Linker`](crate::Linker).
    Call(CallEffect<V>),
    /// Terminate the innermost enclosing body block with carried values
    /// (the message a structured-body frame waits for).
    Yield(Product<V>),
    /// Return from the enclosing function, unwinding inline frames.
    Return(Product<V>),
    /// Run a sub-computation by pushing a dialect-owned `frame`; when it
    /// finishes, its values land in `results`. This is the frame-push /
    /// delegation effect that replaces any framework "enter a scope" concept:
    /// the pushed `frame` is whatever traversal the dialect decided on, opaque
    /// to this enum.
    Push {
        frame: F,
        results: Product<SSAValue>,
    },
}

/// A control-flow edge: target block plus the values for its parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge<V> {
    pub target: Block,
    pub args: Product<V>,
}

impl<V> Edge<V> {
    pub fn new(target: Block, args: Product<V>) -> Self {
        Self { target, args }
    }
}

/// A function invocation request. Resolution is the engine's job, via its
/// [`Linker`](crate::Linker) component.
pub struct CallEffect<V> {
    /// What to call.
    pub callee: Callee,
    /// Optional explicit target stage (e.g. staged calls); defaults to the
    /// caller's stage.
    pub stage: Option<CompileStage>,
    /// Argument values.
    pub args: Product<V>,
    /// Result slots to bind when the callee returns.
    pub results: Product<SSAValue>,
}

/// Callee designators understood by [`Linker`](crate::Linker)s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Callee {
    /// A stage-local symbol, resolved through the symbol table.
    Named(Symbol),
    /// A pipeline-level function.
    Function(Function),
    /// A staged function.
    Staged(StagedFunction),
    /// A fully specialized function.
    Specialized(SpecializedFunction),
}

/// The body a callable statement enters when invoked: a CFG region plus the
/// entry arguments bound to its entry block.
///
/// This is the function-call entry descriptor — the call mechanism, not a
/// structured-control abstraction. A [`FunctionEntry`](crate::FunctionEntry)
/// rule returns one; the engine builds the body frame that walks the region.
pub struct FunctionBody<V> {
    pub region: Region,
    pub args: Product<V>,
}

impl<V> FunctionBody<V> {
    /// A function body over `region`, with no entry arguments yet.
    pub fn new(region: Region) -> Self {
        Self {
            region,
            args: Product::new(),
        }
    }

    /// Entry arguments bound to the region entry block's parameters.
    pub fn args(mut self, args: impl IntoIterator<Item = V>) -> Self {
        self.args = args.into_iter().collect();
        self
    }
}
