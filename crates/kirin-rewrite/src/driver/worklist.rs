use std::collections::{HashSet, VecDeque};

use kirin_ir::walk::WalkOrder;
use kirin_ir::{Dialect, GetInfo, MutationEvent, SSAKind, SSAValue, StageInfo, Statement, Use};

/// The statements a driver run still has to visit, in order.
///
/// A statement is in the worklist at most once: pushing one already waiting
/// does nothing, so it keeps its place. Once popped, it can be pushed again.
pub(super) struct Worklist {
    queue: VecDeque<Statement>,
    waiting: HashSet<Statement>,
}

impl Worklist {
    /// A worklist holding the statements of the walk under `root`, `root`
    /// included, in `O`'s order. The walk is a snapshot: nothing is edited
    /// while it is taken.
    pub(super) fn seeded<O: WalkOrder, L: Dialect>(stage: &StageInfo<L>, root: Statement) -> Self {
        let queue: VecDeque<Statement> = O::schedule(stage, root).into();
        let waiting = queue.iter().copied().collect();
        Self { queue, waiting }
    }

    /// The next statement to visit, or `None` once the worklist is empty.
    pub(super) fn pop(&mut self) -> Option<Statement> {
        let stmt = self.queue.pop_front()?;
        self.waiting.remove(&stmt);
        Some(stmt)
    }

    /// Add `stmt` at the back, unless it is already waiting.
    pub(super) fn push(&mut self, stmt: Statement) {
        if self.waiting.insert(stmt) {
            self.queue.push_back(stmt);
        }
    }

    /// Add the statements `events` may have given a new match: for each
    /// statement an event names as changed, replaced or inserted, its owner,
    /// the producers of its operands, the statement itself and the users of
    /// its results. Other events add nothing; the next sweep visits what
    /// they changed.
    pub(super) fn push_affected<L: Dialect>(
        &mut self,
        stage: &StageInfo<L>,
        events: &[MutationEvent],
    ) {
        for event in events {
            match *event {
                MutationEvent::ChangedOperands { stmt }
                | MutationEvent::ReplacedStatement { stmt }
                | MutationEvent::InsertedStatement { stmt } => self.push_neighbors(stage, stmt),
                _ => {}
            }
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
