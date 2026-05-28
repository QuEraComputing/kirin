use kirin_ir::{
    Block, CompileStage, DiGraph, Function, Region, SpecializedFunction, StagedFunction, Statement,
    UnGraph,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Traversal<T> {
    Entry,
    Active(T),
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    pub stage: CompileStage,
    pub position: Position,
}

impl Location {
    pub fn new(stage: CompileStage, position: Position) -> Self {
        Self { stage, position }
    }

    pub fn active_statement(self) -> Option<Statement> {
        match self.position {
            Position::SpecializedFunction {
                traversal: Traversal::Active(statement),
                ..
            }
            | Position::Block {
                traversal: Traversal::Active(statement),
                ..
            }
            | Position::DiGraph {
                traversal: Traversal::Active(statement),
                ..
            }
            | Position::UnGraph {
                traversal: Traversal::Active(statement),
                ..
            }
            | Position::Statement { statement } => Some(statement),
            _ => None,
        }
    }

    pub fn active_block(self) -> Option<Block> {
        match self.position {
            Position::Region {
                traversal: Traversal::Active(block),
                ..
            } => Some(block),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Position {
    Function {
        function: Function,
        traversal: Traversal<StagedFunction>,
    },
    StagedFunction {
        function: StagedFunction,
        traversal: Traversal<SpecializedFunction>,
    },
    SpecializedFunction {
        function: SpecializedFunction,
        traversal: Traversal<Statement>,
    },
    Region {
        region: Region,
        traversal: Traversal<Block>,
    },
    Block {
        block: Block,
        traversal: Traversal<Statement>,
    },
    Statement {
        statement: Statement,
    },
    DiGraph {
        graph: DiGraph,
        traversal: Traversal<Statement>,
    },
    UnGraph {
        graph: UnGraph,
        traversal: Traversal<Statement>,
    },
}
