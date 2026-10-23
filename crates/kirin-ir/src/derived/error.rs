use std::fmt;

use crate::derived::chain::ChainFinding;
use crate::derived::mirrors::block_body::BlockBody;
use crate::node::ssa::Use;
use crate::{
    Block, CFG, DiGraph, LinkedList, MemberRole, SSAValue, Statement, StatementParent, UnGraph,
};

/// A membership pointer naming a container that is not live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DanglingParent {
    /// `stmt` claims membership of a block that is not live.
    StatementInBlock { stmt: Statement, parent: Block },
    /// `block` claims membership of a CFG that is not live.
    BlockInCFG { block: Block, parent: CFG },
    /// `stmt` claims membership of a `DiGraph` or `UnGraph` that is not live.
    StatementInGraph {
        stmt: Statement,
        parent: StatementParent,
    },
}

impl fmt::Display for DanglingParent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DanglingParent::StatementInBlock { stmt, parent } => write!(
                f,
                "{stmt:?} claims membership of {parent}, which is not live"
            ),
            DanglingParent::BlockInCFG { block, parent } => write!(
                f,
                "{block} claims membership of {parent:?}, which is not live"
            ),
            DanglingParent::StatementInGraph { stmt, parent } => write!(
                f,
                "{stmt:?} claims membership of {parent:?}, which is not live"
            ),
        }
    }
}

/// One defect found in the **authoritative** IR while deriving mirrors.
///
/// A finding means a slot was expecting a live node that is not actually live. Derivation
/// records the finding and carries on, so one scan reports every independent
/// problem rather than just the first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finding {
    /// The `index`-th operand of `stmt` names a value that is not live.
    DanglingOperand {
        stmt: Statement,
        index: usize,
        value: SSAValue,
    },
    /// The `index`-th yield of `graph` names a value that is not live.
    DanglingYield {
        graph: DiGraph,
        index: usize,
        value: SSAValue,
    },
    /// A successor reference on `stmt` targets a block that is not live.
    DanglingSuccessor { stmt: Statement, target: Block },
    /// `stmt` carries successors but is not a terminator. Only a block's
    /// terminator may transfer control, so its edges are not derived.
    NonTerminatorSuccessor { stmt: Statement },
    /// The chain comprising a `Block` or `CFG` has a defect.
    Chain(ChainFinding),
    /// A block has multiple terminator statements
    MultipleTerminators {
        block: Block,
        terminators: Vec<Statement>,
    },
    /// A membership pointer names a container that is not live.
    DanglingParent(DanglingParent),
    /// The wire `value` of `graph` is read by more than two node operand
    /// slots, each listed as `(node, operand_index)`; an undirected edge
    /// joins at most two.
    UnGraphDegreeExceeded {
        graph: UnGraph,
        value: SSAValue,
        occurrences: Vec<(Statement, usize)>,
    },
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Finding::DanglingOperand { stmt, index, value } => write!(
                f,
                "operand {index} of {stmt:?} names {value}, which is not live"
            ),
            Finding::DanglingYield {
                graph,
                index,
                value,
            } => write!(
                f,
                "yield {index} of {graph:?} names {value}, which is not live"
            ),
            Finding::DanglingSuccessor { stmt, target } => write!(
                f,
                "successor of {stmt:?} targets {target}, which is not live"
            ),
            Finding::NonTerminatorSuccessor { stmt } => {
                write!(f, "{stmt:?} carries successors but is not a terminator")
            }
            Finding::Chain(finding) => write!(f, "{finding}"),
            Finding::MultipleTerminators { block, terminators } => write!(
                f,
                "{block} has {} terminators, but a block has at most one: {terminators:?}",
                terminators.len()
            ),
            Finding::DanglingParent(finding) => write!(f, "{finding}"),
            Finding::UnGraphDegreeExceeded {
                graph,
                value,
                occurrences,
            } => write!(
                f,
                "wire {value} of UnGraph {graph} is read by {} node operand slots {occurrences:?}; at most 2 are allowed",
                occurrences.len()
            ),
        }
    }
}

/// The authoritative IR is broken, so the expected mirrors cannot be computed.
///
/// Aggregates every independent [`Finding`] from one scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeriveError {
    findings: Vec<Finding>,
}

impl DeriveError {
    pub(super) fn new(findings: Vec<Finding>) -> Self {
        Self { findings }
    }

    /// Every defect found, in scan order.
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }
}

impl fmt::Display for DeriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "authoritative IR is invalid ({} findings)",
            self.findings.len()
        )?;
        for finding in &self.findings {
            write!(f, "\n  - {finding}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DeriveError {}

/// Installed metadata disagrees with a fresh derivation for one node.
///
/// The authoritative IR is fine; a mutation path failed to keep the mirror in
/// step. Both sides are retained because the difference is the diagnosis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mismatch {
    /// `SSAInfo::uses` disagrees for `value`.
    Uses {
        value: SSAValue,
        installed: Vec<Use>,
        derived: Vec<Use>,
    },
    /// `BlockInfo::predecessors` disagrees for `block`.
    Predecessors {
        block: Block,
        installed: Vec<Block>,
        derived: Vec<Block>,
    },
    /// `BlockInfo::statements` or `BlockInfo::terminator` disagrees for `block`.
    BlockBody {
        block: Block,
        installed: BlockBody,
        derived: BlockBody,
    },
    /// `CFGInfo::blocks` disagrees for `cfg`.
    CFGBlocks {
        cfg: CFG,
        installed: LinkedList<Block>,
        derived: LinkedList<Block>,
    },
    /// The installed members of `graph` differ from the derived ones: one or
    /// more members are missing, extra, or stored with the wrong role. `graph`
    /// is a `StatementParent::DiGraph` or `StatementParent::UnGraph`, and both
    /// lists are sorted by statement.
    GraphMembers {
        graph: StatementParent,
        installed: Vec<(Statement, MemberRole)>,
        derived: Vec<(Statement, MemberRole)>,
    },
    /// The member map and the petgraph of `graph` disagree about the node of
    /// each statement in `statements`: either the map points it at a petgraph
    /// node holding a different statement, or the petgraph has a node for it
    /// that the map does not point at.
    GraphNodeIndex {
        graph: StatementParent,
        statements: Vec<Statement>,
    },
    /// The installed edges of `graph` differ from the derived ones, compared as
    /// multisets of `(a, b, value)` with undirected endpoints in id order.
    GraphEdges {
        graph: StatementParent,
        installed: Vec<(Statement, Statement, SSAValue)>,
        derived: Vec<(Statement, Statement, SSAValue)>,
    },
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mismatch::Uses {
                value,
                installed,
                derived,
            } => write!(
                f,
                "uses of {value}: installed {installed:?}, derived {derived:?}"
            ),
            Mismatch::Predecessors {
                block,
                installed,
                derived,
            } => write!(
                f,
                "predecessors of {block}: installed {installed:?}, derived {derived:?}"
            ),
            Mismatch::BlockBody {
                block,
                installed,
                derived,
            } => write!(
                f,
                "body of {block}: installed {installed:?}, derived {derived:?}"
            ),
            Mismatch::CFGBlocks {
                cfg,
                installed,
                derived,
            } => write!(
                f,
                "blocks of {cfg:?}: installed {installed:?}, derived {derived:?}"
            ),
            Mismatch::GraphMembers {
                graph,
                installed,
                derived,
            } => write!(
                f,
                "members of {graph:?}: installed {installed:?}, derived {derived:?}"
            ),
            Mismatch::GraphNodeIndex { graph, statements } => write!(
                f,
                "member map and petgraph of {graph:?} disagree about the nodes of {statements:?}"
            ),
            Mismatch::GraphEdges {
                graph,
                installed,
                derived,
            } => write!(
                f,
                "edges of {graph:?}: installed {installed:?}, derived {derived:?}"
            ),
        }
    }
}

/// Why [`verify_derived`](super::verify_derived) rejected a stage.
/// [`Derive`](VerifyError::Derive) blames the IR, [`Mismatch`](VerifyError::Mismatch)
/// blames the mutation layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// The authoritative IR is corrupt; no comparison was possible.
    Derive(DeriveError),
    /// Derivation succeeded, but installed metadata disagrees with it.
    Mismatch(Vec<Mismatch>),
}

impl From<DeriveError> for VerifyError {
    fn from(error: DeriveError) -> Self {
        VerifyError::Derive(error)
    }
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::Derive(error) => write!(f, "{error}"),
            VerifyError::Mismatch(mismatches) => {
                write!(
                    f,
                    "derived metadata is stale ({} mismatches); a mutation path is broken",
                    mismatches.len()
                )?;
                for mismatch in mismatches {
                    write!(f, "\n  - {mismatch}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for VerifyError {}
