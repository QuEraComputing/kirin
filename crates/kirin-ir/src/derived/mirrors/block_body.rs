use crate::arena::Id;
use crate::derived::chain::{ChainScan, derive_chains};
use crate::{
    Block, Dialect, Finding, LinkedList, StageInfo, Statement, StatementParent, derived::SlotMap,
};
use crate::{ChainDefect, ChainFinding, DanglingParent, Mismatch};

/// One block's body: the two mirrors that fall out of partitioning its members
/// into the non-terminator chain and the terminator that sits outside it.
///
/// Public because [`Mismatch::BlockBody`](crate::Mismatch) reports one of these
/// per disagreeing block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockBody {
    /// The non-terminator chain summary
    pub statements: LinkedList<Statement>,
    /// The terminator, legitimately absent for a block that has none
    pub terminator: Option<Statement>,
}

pub(in crate::derived) type BlockBodyMap = SlotMap<Block, Option<BlockBody>>;

pub(in crate::derived) fn derive<L: Dialect>(
    stage: &StageInfo<L>,
    findings: &mut Vec<Finding>,
) -> BlockBodyMap {
    let mut block_body_table = BlockBodyMap::sized_like(&stage.nodes.blocks);
    let mut terminator_table: SlotMap<Block, Vec<Statement>> =
        SlotMap::sized_like(&stage.nodes.blocks);

    let mut chain_scan: ChainScan<Statement, Block> =
        ChainScan::new(stage.nodes.statements.len(), stage.nodes.blocks.len());
    for (raw, item) in stage.nodes.statements.items.iter().enumerate() {
        if item.deleted() {
            continue;
        }

        // Statements not inside blocks do not interest us
        let Some(StatementParent::Block(parent)) = item.parent else {
            continue;
        };
        let stmt = Statement::from(Id(raw));

        // Checked once, before the terminator split: both branches record
        // membership, so both would otherwise file the statement under a block
        // that no longer exists.
        if !is_live_block(stage, parent) {
            findings.push(Finding::DanglingParent(DanglingParent::StatementInBlock {
                stmt,
                parent,
            }));
            continue;
        }

        // Collect the terminator into the table. Collected rather than
        // assigned, so `MultipleTerminators` can name every offender.
        if item.definition.is_terminator() {
            terminator_table
                .get_mut(parent)
                .expect("the table is sized like the block arena, and `parent` is a live block")
                .push(stmt);
            continue;
        }

        chain_scan.record(stmt, item.data.node.prev, item.data.node.next, parent);
    }

    let mut defects: Vec<(Block, ChainDefect<Statement>)> = Vec::new();
    let derived_chains = derive_chains(&chain_scan, &mut defects);

    for (block, defect) in defects {
        findings.push(Finding::Chain(ChainFinding::StatementsInBlock(
            defect, block,
        )));
    }

    for (block, mirror) in derived_chains.iter() {
        // Extract the terminator from the table. Report if multiple termintors were found.
        let terminators: &[Statement] = terminator_table.get(block).map_or(&[], |t| t.as_slice());
        let terminator = match terminators {
            [] => None,
            &[only] => Some(only),
            _ => {
                findings.push(Finding::MultipleTerminators {
                    block,
                    terminators: terminators.to_vec(),
                });
                continue;
            }
        };

        let block_body = block_body_table
            .get_mut(block)
            .expect("both tables are sized like the block arena");
        *block_body = mirror.map(|statements| BlockBody {
            statements,
            terminator,
        });
    }

    block_body_table
}

impl BlockBodyMap {
    /// Write both fields into every block whose body derived. Finalization only.
    ///
    /// A block that did not derive keeps whatever mirror it already had.
    /// This will only occur when using [`install_and_derive_mirrors_unchecked`](crate::derived::install_and_derive_mirrors_unchecked)
    pub(in crate::derived) fn install<L: Dialect>(self, stage: &mut StageInfo<L>) {
        assert!(self.len() == stage.nodes.blocks.len());
        let stage_blocks = stage.nodes.blocks.items.iter_mut();
        for (item, block_body_opt) in stage_blocks.zip(self.into_iter()) {
            let Some(block_body) = block_body_opt else {
                continue;
            };
            item.terminator = block_body.terminator;
            item.statements = block_body.statements;
        }
    }

    /// Compare the installed body with this derivation, appending one
    /// [`Mismatch`] per disagreeing block. Never writes to the stage.
    pub(in crate::derived) fn verify<L: Dialect>(
        &self,
        stage: &StageInfo<L>,
        out: &mut Vec<Mismatch>,
    ) {
        for (raw, item) in stage.nodes.blocks.items.iter().enumerate() {
            if item.deleted() {
                continue;
            }
            let block = Block::from(Id(raw));

            // A block whose body could not be derived has nothing to compare
            // against; the defect was already reported as a `Finding`.
            let Some(derived) = self.get(block).and_then(Option::as_ref) else {
                continue;
            };
            let installed = BlockBody {
                statements: item.statements,
                terminator: item.terminator,
            };

            if *derived != installed {
                out.push(Mismatch::BlockBody {
                    block,
                    installed,
                    derived: derived.clone(),
                });
            }
        }
    }
}

/// Whether `block` resolves to a live block of `stage`.
fn is_live_block<L: Dialect>(stage: &StageInfo<L>, block: Block) -> bool {
    stage
        .nodes
        .blocks
        .get(block)
        .is_some_and(|item| !item.deleted())
}
