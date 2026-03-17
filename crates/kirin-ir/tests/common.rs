//! Unified test dialect for kirin-ir integration tests.
//!
//! `BuilderDialect` consolidates the capabilities of all previous test dialects
//! (TestDialect, RichDialect, GraphDialect, UgDialect) into a single enum that
//! covers: arguments, results, terminators, and edges.

use kirin_ir::*;

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum TestType {
    #[default]
    Any,
    I32,
    I64,
    Qubit,
}

impl std::fmt::Display for TestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestType::Any => write!(f, "any"),
            TestType::I32 => write!(f, "i32"),
            TestType::I64 => write!(f, "i64"),
            TestType::Qubit => write!(f, "qubit"),
        }
    }
}

impl Placeholder for TestType {
    fn placeholder() -> Self {
        Self::Any
    }
}

/// A unified dialect covering all builder test scenarios:
///
/// - `Nop`: no operands, no results
/// - `Return`: terminator
/// - `Add(a, b)`: two SSAValue operands (block arg substitution, digraph edges)
/// - `Use(a)`: one SSAValue operand (ungraph node)
/// - `Gate(a, b)`: two SSAValue operands (ungraph node)
/// - `Wire(r)`: edge that produces a ResultValue (ungraph edge)
/// - `Isolated`: no operands, no results (ungraph isolated node)
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BuilderDialect {
    Nop,
    Return,
    Add(SSAValue, SSAValue),
    Use(SSAValue),
    Gate(SSAValue, SSAValue),
    Wire(ResultValue),
    Isolated,
}

impl<'a> HasArguments<'a> for BuilderDialect {
    type Iter = std::vec::IntoIter<&'a SSAValue>;
    fn arguments(&'a self) -> Self::Iter {
        match self {
            BuilderDialect::Add(a, b) | BuilderDialect::Gate(a, b) => vec![a, b].into_iter(),
            BuilderDialect::Use(a) => vec![a].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasArgumentsMut<'a> for BuilderDialect {
    type IterMut = std::vec::IntoIter<&'a mut SSAValue>;
    fn arguments_mut(&'a mut self) -> Self::IterMut {
        match self {
            BuilderDialect::Add(a, b) | BuilderDialect::Gate(a, b) => vec![a, b].into_iter(),
            BuilderDialect::Use(a) => vec![a].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasResults<'a> for BuilderDialect {
    type Iter = std::vec::IntoIter<&'a ResultValue>;
    fn results(&'a self) -> Self::Iter {
        match self {
            BuilderDialect::Wire(r) => vec![r].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasResultsMut<'a> for BuilderDialect {
    type IterMut = std::vec::IntoIter<&'a mut ResultValue>;
    fn results_mut(&'a mut self) -> Self::IterMut {
        match self {
            BuilderDialect::Wire(r) => vec![r].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasBlocks<'a> for BuilderDialect {
    type Iter = std::iter::Empty<&'a Block>;
    fn blocks(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasBlocksMut<'a> for BuilderDialect {
    type IterMut = std::iter::Empty<&'a mut Block>;
    fn blocks_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasSuccessors<'a> for BuilderDialect {
    type Iter = std::iter::Empty<&'a Successor>;
    fn successors(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasSuccessorsMut<'a> for BuilderDialect {
    type IterMut = std::iter::Empty<&'a mut Successor>;
    fn successors_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasRegions<'a> for BuilderDialect {
    type Iter = std::iter::Empty<&'a Region>;
    fn regions(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasRegionsMut<'a> for BuilderDialect {
    type IterMut = std::iter::Empty<&'a mut Region>;
    fn regions_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl IsTerminator for BuilderDialect {
    fn is_terminator(&self) -> bool {
        matches!(self, BuilderDialect::Return)
    }
}

impl IsConstant for BuilderDialect {
    fn is_constant(&self) -> bool {
        false
    }
}

impl IsPure for BuilderDialect {
    fn is_pure(&self) -> bool {
        true
    }
}

impl IsSpeculatable for BuilderDialect {
    fn is_speculatable(&self) -> bool {
        true
    }
}

impl<'a> HasDigraphs<'a> for BuilderDialect {
    type Iter = std::iter::Empty<&'a DiGraph>;
    fn digraphs(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasDigraphsMut<'a> for BuilderDialect {
    type IterMut = std::iter::Empty<&'a mut DiGraph>;
    fn digraphs_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasUngraphs<'a> for BuilderDialect {
    type Iter = std::iter::Empty<&'a UnGraph>;
    fn ungraphs(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasUngraphsMut<'a> for BuilderDialect {
    type IterMut = std::iter::Empty<&'a mut UnGraph>;
    fn ungraphs_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl IsEdge for BuilderDialect {
    fn is_edge(&self) -> bool {
        matches!(self, BuilderDialect::Wire(_))
    }
}

impl Dialect for BuilderDialect {
    type Type = TestType;
}

/// Create a Wire edge statement that produces a ResultValue.
///
/// Uses `ssa_arena().next_id()` to peek at the next SSA id, creates the Wire
/// statement with that id as its ResultValue, then allocates the SSA result
/// via the public `ssa()` builder.
#[allow(dead_code)]
pub fn make_wire(stage: &mut StageInfo<BuilderDialect>) -> (Statement, SSAValue) {
    let result_id: ResultValue = stage.ssa_arena().next_id().into();
    let stmt = stage
        .statement()
        .definition(BuilderDialect::Wire(result_id))
        .new();
    let wire_ssa = stage
        .ssa()
        .ty(TestType::Qubit)
        .kind(SSAKind::Result(stmt, 0))
        .new();
    (stmt, wire_ssa)
}

/// Create a new StageInfo with the unified BuilderDialect.
#[allow(dead_code)]
pub fn new_stage() -> StageInfo<BuilderDialect> {
    StageInfo::default()
}
