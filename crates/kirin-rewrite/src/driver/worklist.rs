use std::collections::{HashMap, VecDeque};

use kirin_ir::walk::WalkOrder;
use kirin_ir::{Dialect, GetInfo, MutationEvent, SSAKind, SSAValue, StageInfo, Statement, Use};

/// The statements a driver sweep still has to visit, in order.
///
/// Only statements under the sweep's root are ever added: those of the walk
/// it starts from, and those inserted under one of them. A statement is in
/// the queue at most once: pushing one already in it does nothing, so it
/// keeps its place. Once popped, it can be pushed again.
pub(super) struct Worklist {
    queue: VecDeque<Statement>,
    /// The statements under the root, each with its status.
    scope: HashMap<Statement, Status>,
}

/// Whether a statement under the root is in the worklist's queue.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    /// In the queue, to be visited.
    InQueue,
    /// Not in the queue: visited already, or inserted under the root and
    /// not added yet.
    VisitedOrInserted,
}

impl Worklist {
    /// A worklist holding the statements of the walk under `root`, `root`
    /// included, in `O`'s order. The walk is a snapshot: nothing is edited
    /// while it is taken.
    pub(super) fn seeded<O: WalkOrder, L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Self {
        let queue: VecDeque<Statement> = O::schedule(stage, root).into();
        let scope = queue.iter().map(|&stmt| (stmt, Status::InQueue)).collect();
        Self { queue, scope }
    }

    /// The next statement to visit, or `None` once the worklist is empty.
    pub(super) fn pop(&mut self) -> Option<Statement> {
        let stmt = self.queue.pop_front()?;
        self.scope.insert(stmt, Status::VisitedOrInserted);
        Some(stmt)
    }

    /// Add `stmt` at the back of the queue, unless it is already in it or is
    /// not under the root.
    pub(super) fn push(&mut self, stmt: Statement) {
        if let Some(status) = self.scope.get_mut(&stmt)
            && *status == Status::VisitedOrInserted
        {
            *status = Status::InQueue;
            self.queue.push_back(stmt);
        }
    }

    /// Add the statements `events` may have given a new match: for each
    /// statement an event names as changed, replaced or inserted, its owner,
    /// the producers of its operands, the statement itself and the users of
    /// its results, those of them under the root. Other events add nothing;
    /// the next sweep visits what they changed.
    pub(super) fn push_affected<L: Dialect>(
        &mut self,
        stage: &StageInfo<L>,
        events: &[MutationEvent],
    ) {
        for event in events {
            match *event {
                MutationEvent::InsertedStatement { stmt } => {
                    self.adopt(stage, stmt);
                    self.push_neighbors(stage, stmt);
                }
                MutationEvent::ChangedOperands { stmt }
                | MutationEvent::ReplacedStatement { stmt } => self.push_neighbors(stage, stmt),
                _ => {}
            }
        }
    }

    /// Count the inserted `stmt` under the root if its owner is.
    fn adopt<L: Dialect>(&mut self, stage: &StageInfo<L>, stmt: Statement) {
        if let Some(owner) = stmt.owner(stage)
            && self.scope.contains_key(&owner)
        {
            self.scope.entry(stmt).or_insert(Status::VisitedOrInserted);
        }
    }

    /// Add `stmt`'s owner, the producers of its operands, `stmt` itself and
    /// the users of its results, top-down, as the walk visits them. Nothing
    /// if `stmt` is not live.
    fn push_neighbors<L: Dialect>(&mut self, stage: &StageInfo<L>, stmt: Statement) {
        let Some(info) = stmt.get_info(stage) else {
            return;
        };
        if let Some(owner) = stmt.owner(stage) {
            self.push(owner);
        }
        let definition = info.definition();
        for &operand in definition.arguments() {
            if let Some(SSAKind::Result(producer, _)) = operand.get_info(stage).map(|v| *v.kind()) {
                self.push(producer);
            }
        }
        self.push(stmt);
        for &result in definition.results() {
            let Some(value) = SSAValue::from(result).get_info(stage) else {
                continue;
            };
            for site in value.uses() {
                if let Use::StatementOperand { stmt: user, .. } = *site {
                    self.push(user);
                }
            }
        }
    }
}
