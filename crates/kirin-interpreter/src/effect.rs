use kirin_ir::{
    Block, CompileStage, Function, Product, Region, SSAValue, SpecializedFunction, StagedFunction,
    Symbol,
};

use crate::EnvOps;

/// The closed control algebra a statement can produce.
///
/// Atomic statements read operands, write results, and return [`Effect::Next`].
/// Control statements name successor edges, calls, or scopes; the engine owns
/// how each is driven. Undecided variants ([`Effect::Branch`],
/// [`Effect::EnterAny`]) are errors under concrete execution and explored
/// exhaustively (then joined) under abstract interpretation — dialects emit
/// them based on the *value* (`BranchCondition::is_truthy` returning `None`),
/// never based on which engine is running.
pub enum Effect<V, E> {
    /// Statement done; continue with the next statement.
    Next,
    /// Unconditional transfer to a block in the current region.
    Jump(Edge<V>),
    /// Conditional transfer whose condition is undecided in the value domain.
    Branch(Vec<Edge<V>>),
    /// Invoke a function through the engine's [`Linker`](crate::Linker).
    Call(CallEffect<V>),
    /// Terminate the innermost enclosing scope body with carried values.
    Yield(Product<V>),
    /// Return from the enclosing function, unwinding inline scopes.
    Return(Product<V>),
    /// Run a structured sub-computation (e.g. an `scf.if`/`scf.for` body).
    Enter(Scope<V, E>),
    /// Run one of several scopes whose selector is undecided; results join.
    EnterAny(Vec<Scope<V, E>>),
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

/// A structured sub-computation: a body, entry arguments, result bindings,
/// and an optional [`ScopeHook`] deciding what happens when the body yields.
///
/// Without a hook, the first yield finishes the scope with the yielded values
/// (the `scf.if` shape). With a hook, the dialect decides whether to repeat
/// the body (the `scf.for` shape).
pub struct Scope<V, E> {
    pub(crate) body: ScopeBody,
    pub(crate) args: Product<V>,
    pub(crate) results: Product<SSAValue>,
    pub(crate) hook: Option<Box<dyn ScopeHook<V, E>>>,
}

/// What a [`Scope`] executes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeBody {
    /// A single block terminated by a yield.
    Block(Block),
    /// A multi-block CFG region (function bodies).
    Region(Region),
    /// No body: finish immediately with the scope's `args` as results.
    Immediate,
}

impl<V, E> Scope<V, E> {
    /// Scope over a single body block (scf-style).
    pub fn block(body: Block) -> Self {
        Self {
            body: ScopeBody::Block(body),
            args: Product::new(),
            results: Product::new(),
            hook: None,
        }
    }

    /// Scope over a multi-block region (function bodies).
    pub fn region(body: Region) -> Self {
        Self {
            body: ScopeBody::Region(body),
            args: Product::new(),
            results: Product::new(),
            hook: None,
        }
    }

    /// Scope that finishes immediately with `values` as its results.
    /// Useful as an [`Effect::EnterAny`] alternative for "skip" paths.
    pub fn immediate(values: Product<V>) -> Self {
        Self {
            body: ScopeBody::Immediate,
            args: values,
            results: Product::new(),
            hook: None,
        }
    }

    /// Entry arguments bound to the body block's parameters.
    pub fn args(mut self, args: impl IntoIterator<Item = V>) -> Self {
        self.args = args.into_iter().collect();
        self
    }

    /// Result slots written when the scope finishes.
    pub fn bind<T: Into<SSAValue>>(mut self, results: impl IntoIterator<Item = T>) -> Self {
        self.results = results.into_iter().map(Into::into).collect();
        self
    }

    /// Install a hook deciding what happens when the body yields.
    pub fn on_yield(mut self, hook: impl ScopeHook<V, E> + 'static) -> Self {
        self.hook = Some(Box::new(hook));
        self
    }

    pub fn body(&self) -> ScopeBody {
        self.body
    }
}

/// Dialect-side policy for a yielding scope body.
///
/// Called when the scope body terminates with a yield. `entry` is the
/// product currently bound to the body's parameters (under abstract
/// interpretation this is the joined entry state, so hooks should derive
/// iteration state from it rather than from captured per-iteration values).
pub trait ScopeHook<V, E> {
    fn on_yield(
        self: Box<Self>,
        entry: &Product<V>,
        yielded: Product<V>,
        env: &mut dyn EnvOps<V, E>,
    ) -> Result<ScopeStep<V, E>, E>;
}

/// A [`ScopeHook`]'s verdict after a yield.
pub enum ScopeStep<V, E> {
    /// The scope is done; write `Finish.0` to its result bindings.
    Finish(Product<V>),
    /// Re-enter the body with new entry arguments.
    Repeat {
        args: Product<V>,
        hook: Box<dyn ScopeHook<V, E>>,
    },
    /// The continue/finish condition is undecided in the value domain.
    /// Concrete engines reject this; abstract engines explore both.
    RepeatOrFinish {
        args: Product<V>,
        results: Product<V>,
        hook: Box<dyn ScopeHook<V, E>>,
    },
}
