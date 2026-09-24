use std::fmt;

use crate::derived::chain::ChainFinding;
use crate::derived::mirrors::block_body::BlockBody;
use crate::node::ssa::Use;
use crate::{Block, CFG, DiGraph, LinkedList, SSAValue, Statement};

/// A membership pointer naming a container that is not live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DanglingParent {
    /// `stmt` claims membership of a block that is not live.
    StatementInBlock { stmt: Statement, parent: Block },
    /// `block` claims membership of a CFG that is not live.
    BlockInCFG { block: Block, parent: CFG },
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
    /// The chain comprising a `Block` or `CFG` has a defect.
    Chain(ChainFinding),
    /// A block has multiple terminator statements
    MultipleTerminators {
        block: Block,
        terminators: Vec<Statement>,
    },
    /// A membership pointer names a container that is not live.
    DanglingParent(DanglingParent),
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
            Finding::Chain(finding) => write!(f, "{finding}"),
            Finding::MultipleTerminators { block, terminators } => write!(
                f,
                "{block} has {} terminators, but a block has at most one: {terminators:?}",
                terminators.len()
            ),
            Finding::DanglingParent(finding) => write!(f, "{finding}"),
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
