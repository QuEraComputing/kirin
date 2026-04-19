use kirin_ir::{CompileStage, ResultValue, SpecializedFunction};

/// A node in the abstract call graph, recording a single call site.
///
/// `AbstractInterp` accumulates these into a graph
/// (`call_graph: FxHashMap<StagedKey, FxHashSet<AbstractFrame>>`) that maps
/// each callee to the set of call sites that can invoke it.  When a callee's
/// output summary grows, the graph is used to re-enqueue every affected caller.
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct AbstractFrame {
    pub func: SpecializedFunction,
    pub stage: CompileStage,
    /// SSA results of the call instruction — unique within `func`.
    pub results: Vec<ResultValue>,
}
