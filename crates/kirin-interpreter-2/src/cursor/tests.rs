use kirin_ir::{
    Block, BuilderSSAKind, BuilderStageInfo, CompileStage, DiGraph, Dialect, GetInfo, HasArguments,
    HasArgumentsMut, HasBlocks, HasBlocksMut, HasDigraphs, HasDigraphsMut, HasRegions,
    HasRegionsMut, HasResults, HasResultsMut, HasSuccessors, HasSuccessorsMut, HasUngraphs,
    HasUngraphsMut, IsConstant, IsEdge, IsPure, IsSpeculatable, IsTerminator, Pipeline, Region,
    ResultValue, SSAValue, StageInfo, Statement, Successor, UnGraph,
};
use kirin_test_languages::SimpleType;

use crate::{
    BlockSeed, DiGraphSeed, ExecutionSeed, RegionSeed, UnGraphSeed,
    cursor::{DiGraphCursor, ExecutionCursor, RegionCursor, UnGraphCursor},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum CursorDialect {
    Nop,
    Wire(ResultValue),
    Use(SSAValue),
    Gate(SSAValue, SSAValue),
}

impl Dialect for CursorDialect {
    type Type = SimpleType;
}

impl<'a> HasArguments<'a> for CursorDialect {
    type Iter = std::vec::IntoIter<&'a SSAValue>;

    fn arguments(&'a self) -> Self::Iter {
        match self {
            Self::Use(value) => vec![value].into_iter(),
            Self::Gate(lhs, rhs) => vec![lhs, rhs].into_iter(),
            _ => Vec::new().into_iter(),
        }
    }
}

impl<'a> HasArgumentsMut<'a> for CursorDialect {
    type IterMut = std::vec::IntoIter<&'a mut SSAValue>;

    fn arguments_mut(&'a mut self) -> Self::IterMut {
        match self {
            Self::Use(value) => vec![value].into_iter(),
            Self::Gate(lhs, rhs) => vec![lhs, rhs].into_iter(),
            _ => Vec::new().into_iter(),
        }
    }
}

impl<'a> HasResults<'a> for CursorDialect {
    type Iter = std::vec::IntoIter<&'a ResultValue>;

    fn results(&'a self) -> Self::Iter {
        match self {
            Self::Wire(result) => vec![result].into_iter(),
            _ => Vec::new().into_iter(),
        }
    }
}

impl<'a> HasResultsMut<'a> for CursorDialect {
    type IterMut = std::vec::IntoIter<&'a mut ResultValue>;

    fn results_mut(&'a mut self) -> Self::IterMut {
        match self {
            Self::Wire(result) => vec![result].into_iter(),
            _ => Vec::new().into_iter(),
        }
    }
}

impl<'a> HasBlocks<'a> for CursorDialect {
    type Iter = std::iter::Empty<&'a Block>;

    fn blocks(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasBlocksMut<'a> for CursorDialect {
    type IterMut = std::iter::Empty<&'a mut Block>;

    fn blocks_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasSuccessors<'a> for CursorDialect {
    type Iter = std::iter::Empty<&'a Successor>;

    fn successors(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasSuccessorsMut<'a> for CursorDialect {
    type IterMut = std::iter::Empty<&'a mut Successor>;

    fn successors_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasRegions<'a> for CursorDialect {
    type Iter = std::iter::Empty<&'a Region>;

    fn regions(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasRegionsMut<'a> for CursorDialect {
    type IterMut = std::iter::Empty<&'a mut Region>;

    fn regions_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasDigraphs<'a> for CursorDialect {
    type Iter = std::iter::Empty<&'a DiGraph>;

    fn digraphs(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasDigraphsMut<'a> for CursorDialect {
    type IterMut = std::iter::Empty<&'a mut DiGraph>;

    fn digraphs_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl<'a> HasUngraphs<'a> for CursorDialect {
    type Iter = std::iter::Empty<&'a UnGraph>;

    fn ungraphs(&'a self) -> Self::Iter {
        std::iter::empty()
    }
}

impl<'a> HasUngraphsMut<'a> for CursorDialect {
    type IterMut = std::iter::Empty<&'a mut UnGraph>;

    fn ungraphs_mut(&'a mut self) -> Self::IterMut {
        std::iter::empty()
    }
}

impl IsTerminator for CursorDialect {
    fn is_terminator(&self) -> bool {
        false
    }
}

impl IsConstant for CursorDialect {
    fn is_constant(&self) -> bool {
        false
    }
}

impl IsPure for CursorDialect {
    fn is_pure(&self) -> bool {
        true
    }
}

impl IsSpeculatable for CursorDialect {
    fn is_speculatable(&self) -> bool {
        true
    }
}

impl IsEdge for CursorDialect {
    fn is_edge(&self) -> bool {
        matches!(self, Self::Wire(_))
    }
}

fn empty_stage() -> BuilderStageInfo<CursorDialect> {
    BuilderStageInfo::default()
}

fn make_nop(stage: &mut BuilderStageInfo<CursorDialect>) -> Statement {
    stage.statement().definition(CursorDialect::Nop).new()
}

fn make_wire(stage: &mut BuilderStageInfo<CursorDialect>) -> (Statement, SSAValue) {
    let result_id: ResultValue = stage.ssa_arena().next_id().into();
    let stmt = stage
        .statement()
        .definition(CursorDialect::Wire(result_id))
        .new();
    let ssa = stage
        .ssa()
        .ty(SimpleType::Any)
        .kind(BuilderSSAKind::Result(stmt, 0))
        .new();
    (stmt, ssa)
}

fn make_use(stage: &mut BuilderStageInfo<CursorDialect>, value: SSAValue) -> Statement {
    stage
        .statement()
        .definition(CursorDialect::Use(value))
        .new()
}

fn make_gate(
    stage: &mut BuilderStageInfo<CursorDialect>,
    lhs: SSAValue,
    rhs: SSAValue,
) -> Statement {
    stage
        .statement()
        .definition(CursorDialect::Gate(lhs, rhs))
        .new()
}

fn first_stage_id(pipeline: &mut Pipeline<StageInfo<CursorDialect>>) -> CompileStage {
    pipeline.add_stage().stage(StageInfo::default()).new()
}

#[test]
fn region_cursor_walks_non_empty_blocks_in_region_order() {
    let mut pipeline: Pipeline<StageInfo<CursorDialect>> = Pipeline::new();
    let stage_id = first_stage_id(&mut pipeline);

    let (region, first_stmt, second_stmt) =
        pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
            let first = make_nop(b);
            let second = make_nop(b);
            let block0 = b.block().stmt(first).new();
            let empty = b.block().new();
            let block1 = b.block().stmt(second).new();
            let region = b
                .region()
                .add_block(block0)
                .add_block(empty)
                .add_block(block1)
                .new();
            (region, first, second)
        });

    let stage = pipeline.stage(stage_id).unwrap();
    let mut cursor = RegionCursor::new(stage, region);

    assert_eq!(cursor.current(), Some(first_stmt));
    cursor.advance(stage);
    assert_eq!(cursor.current(), Some(second_stmt));
    cursor.advance(stage);
    assert_eq!(cursor.current(), None);
}

#[test]
fn digraph_cursor_follows_stored_node_order() {
    let mut pipeline: Pipeline<StageInfo<CursorDialect>> = Pipeline::new();
    let stage_id = first_stage_id(&mut pipeline);

    let (digraph, first, second) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let first = make_nop(b);
        let second = make_nop(b);
        let digraph = b.digraph().node(first).node(second).new();
        (digraph, first, second)
    });

    let stage = pipeline.stage(stage_id).unwrap();
    let mut cursor = DiGraphCursor::new(stage, digraph);

    assert_eq!(cursor.current(), Some(first));
    cursor.advance();
    assert_eq!(cursor.current(), Some(second));
    cursor.advance();
    assert_eq!(cursor.current(), None);
}

#[test]
fn ungraph_cursor_follows_bfs_canonical_node_order() {
    let mut pipeline: Pipeline<StageInfo<CursorDialect>> = Pipeline::new();
    let stage_id = first_stage_id(&mut pipeline);

    let (ungraph, near, mid, far) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let port_placeholder = b.graph_port().index(0);
        let (wire0_stmt, wire0) = make_wire(b);
        let (wire1_stmt, wire1) = make_wire(b);

        let far = make_use(b, wire0);
        let mid = make_gate(b, wire0, wire1);
        let near = make_gate(b, wire1, port_placeholder);

        let ungraph = b
            .ungraph()
            .port(SimpleType::Any)
            .edge(wire0_stmt)
            .edge(wire1_stmt)
            .node(far)
            .node(mid)
            .node(near)
            .new();
        (ungraph, near, mid, far)
    });

    let stage = pipeline.stage(stage_id).unwrap();
    let info = ungraph.expect_info(stage);
    let node_order: Vec<_> = info
        .graph()
        .node_indices()
        .map(|node| info.graph()[node])
        .collect();
    assert_eq!(node_order, vec![near, mid, far]);

    let mut cursor = UnGraphCursor::new(stage, ungraph);
    assert_eq!(cursor.current(), Some(near));
    cursor.advance();
    assert_eq!(cursor.current(), Some(mid));
    cursor.advance();
    assert_eq!(cursor.current(), Some(far));
    cursor.advance();
    assert_eq!(cursor.current(), None);
}

#[test]
fn execution_cursor_construction_matches_seed_shape() {
    let mut pipeline: Pipeline<StageInfo<CursorDialect>> = Pipeline::new();
    let stage_id = first_stage_id(&mut pipeline);

    let (block, region, digraph, ungraph) =
        pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
            let block_stmt = make_nop(b);
            let block = b.block().stmt(block_stmt).new();

            let region_stmt = make_nop(b);
            let region_block = b.block().stmt(region_stmt).new();
            let region = b.region().add_block(region_block).new();

            let digraph_stmt = make_nop(b);
            let digraph = b.digraph().node(digraph_stmt).new();

            let port_placeholder = b.graph_port().index(0);
            let (wire_stmt, _) = make_wire(b);
            let ungraph_stmt = make_use(b, port_placeholder);
            let ungraph = b
                .ungraph()
                .port(SimpleType::Any)
                .edge(wire_stmt)
                .node(ungraph_stmt)
                .new();

            (block, region, digraph, ungraph)
        });

    let stage = pipeline.stage(stage_id).unwrap();

    assert!(matches!(
        ExecutionCursor::from_seed(stage, ExecutionSeed::from(BlockSeed::new(block))),
        ExecutionCursor::Block(_)
    ));
    assert!(matches!(
        ExecutionCursor::from_seed(stage, ExecutionSeed::from(RegionSeed::new(region))),
        ExecutionCursor::Region(_)
    ));
    assert!(matches!(
        ExecutionCursor::from_seed(stage, ExecutionSeed::from(DiGraphSeed::new(digraph))),
        ExecutionCursor::DiGraph(_)
    ));
    assert!(matches!(
        ExecutionCursor::from_seed(stage, ExecutionSeed::from(UnGraphSeed::new(ungraph))),
        ExecutionCursor::UnGraph(_)
    ));
}

#[test]
fn execution_cursor_current_block_is_shape_specific() {
    let mut stage = empty_stage();
    let (block, region, digraph, ungraph) = {
        let block_stmt = make_nop(&mut stage);
        let block = stage.block().stmt(block_stmt).new();

        let region_stmt = make_nop(&mut stage);
        let region_block = stage.block().stmt(region_stmt).new();
        let region = stage.region().add_block(region_block).new();

        let digraph_stmt = make_nop(&mut stage);
        let digraph = stage.digraph().node(digraph_stmt).new();

        let port_placeholder = stage.graph_port().index(0);
        let (wire_stmt, _) = make_wire(&mut stage);
        let ungraph_stmt = make_use(&mut stage, port_placeholder);
        let ungraph = stage
            .ungraph()
            .port(SimpleType::Any)
            .edge(wire_stmt)
            .node(ungraph_stmt)
            .new();

        (block, region, digraph, ungraph)
    };
    let stage = stage.finalize().unwrap();

    let block_cursor = ExecutionCursor::from_seed(&stage, ExecutionSeed::from(block));
    let region_cursor = ExecutionCursor::from_seed(&stage, ExecutionSeed::from(region));
    let digraph_cursor = ExecutionCursor::from_seed(&stage, ExecutionSeed::from(digraph));
    let ungraph_cursor = ExecutionCursor::from_seed(&stage, ExecutionSeed::from(ungraph));

    assert_eq!(block_cursor.current_block(), Some(block));
    assert_eq!(
        region_cursor.current_block(),
        Some(region.blocks(&stage).next().unwrap())
    );
    assert_eq!(digraph_cursor.current_block(), None);
    assert_eq!(ungraph_cursor.current_block(), None);
}
