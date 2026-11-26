use std::collections::{HashSet, VecDeque};

use kirin_ir::walk::WalkOrder;
use kirin_ir::{
    Dialect, GetInfo, MutationEvent, SSAKind, SSAValue, StageInfo, Statement, StatementParent, Use,
};

/// The statements a driver sweep still has to visit, in order.
///
/// Only statements under the sweep's root are ever added: those of the walk
/// it starts from, and those inserted under one of them. A statement is in
/// the queue at most once: pushing one already in it does nothing, so it
/// keeps its place. Once popped, it can be pushed again.
///
/// One worklist serves every sweep of a run, and every root of
/// [`RewriteDriver::run`](super::RewriteDriver::run): each statement's
/// status is kept at its arena index in one vector, which grows with the
/// arena and is never reallocated per sweep, and seeding a sweep resets
/// only the statuses the previous sweep set.
#[derive(Default)]
pub(super) struct Worklist {
    /// The walk the sweep starts from, in order.
    snapshot: Vec<Statement>,
    /// The index in `snapshot` of the next statement to visit.
    next: usize,
    /// The statements added since the sweep began, after the rest of
    /// `snapshot`.
    queue: VecDeque<Statement>,
    /// The statements inserted under the root during the sweep.
    inserted: Vec<Statement>,
    /// Each statement's status.
    status: Statuses,
    /// The sweep's root.
    root: Option<Statement>,
    /// Whether the sweep tries native rules, which may read anything around
    /// a statement, so an edit also adds owners and producers.
    native_rules_present: bool,
    /// Whether an edit since the sweep began moved statements somewhere not
    /// under the root, so a queued statement may have left the root's tree.
    move_occurred: bool,
}

/// Where a statement stands in the worklist.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Status {
    /// Not under the root.
    #[default]
    Outside,
    /// In the queue, to be visited.
    InQueue,
    /// Under the root but not in the queue: visited already, or inserted
    /// and not added yet.
    VisitedOrInserted,
}

/// Each statement's [`Status`], at its arena index. A statement past the
/// end is [`Status::Outside`].
#[derive(Default)]
struct Statuses(Vec<Status>);

impl Statuses {
    fn get(&self, stmt: Statement) -> Status {
        self.0.get(stmt.id().raw()).copied().unwrap_or_default()
    }

    /// Set `stmt`'s status, growing the vector if `stmt` is past its end.
    fn set(&mut self, stmt: Statement, status: Status) {
        let index = stmt.id().raw();
        if self.0.len() <= index {
            self.0.resize(index + 1, Status::Outside);
        }
        self.0[index] = status;
    }
}

impl Worklist {
    /// Start a sweep from the statements of the walk under `root`, `root`
    /// included, in `O`'s order, trying native rules if
    /// `native_rules_present`. The walk is a snapshot: nothing is edited
    /// while it is taken. The statuses the previous sweep set go back to
    /// [`Status::Outside`], and the statements it left in the queue, after
    /// an `interrupt`, are dropped.
    pub(super) fn seed<O: WalkOrder, L: Dialect>(
        &mut self,
        stage: &StageInfo<L>,
        root: Statement,
        native_rules_present: bool,
    ) {
        for &stmt in self.snapshot.iter().chain(&self.inserted) {
            self.status.set(stmt, Status::Outside);
        }
        self.snapshot = O::schedule(stage, root);
        self.next = 0;
        self.queue.clear();
        self.inserted.clear();
        for &stmt in &self.snapshot {
            self.status.set(stmt, Status::InQueue);
        }
        self.root = Some(root);
        self.native_rules_present = native_rules_present;
        self.move_occurred = false;
    }

    /// Whether an edit since the sweep began moved statements somewhere not
    /// under the root, so a queued statement may no longer be under it.
    pub(super) fn move_occurred(&self) -> bool {
        self.move_occurred
    }

    /// The next statement to visit, or `None` once the worklist is empty.
    pub(super) fn pop(&mut self) -> Option<Statement> {
        let stmt = match self.snapshot.get(self.next) {
            Some(&stmt) => {
                self.next += 1;
                stmt
            }
            None => self.queue.pop_front()?,
        };
        self.status.set(stmt, Status::VisitedOrInserted);
        Some(stmt)
    }

    /// Add `stmt` at the back of the queue, unless it is already in it or is
    /// not under the root.
    pub(super) fn push(&mut self, stmt: Statement) {
        if self.status.get(stmt) == Status::VisitedOrInserted {
            self.status.set(stmt, Status::InQueue);
            self.queue.push_back(stmt);
        }
    }

    /// Add the statements `events` may have given a new match: for each
    /// statement an event names as changed, replaced, created or newly
    /// placed, the statement itself and the users of its results, and with
    /// native rules also its owner and the producers of its operands, those
    /// of them under the root. A [`MutationEvent::PlacedStatement`] for a
    /// statement not under the root yet (a detached statement `insert`
    /// placed, or one moved in from outside the root) counts as newly placed,
    /// as a created one does; for one already under the root it is a move. A
    /// move or splice adds nothing. It is recorded (see
    /// [`move_occurred`](Self::move_occurred)) only if it took statements out
    /// of the root's tree, checked once per destination: a splice, split or
    /// merge costs one walk up from the block it moved statements into, not
    /// one per statement.
    /// Other events add nothing; the next sweep visits what they changed.
    pub(super) fn push_affected<L: Dialect>(
        &mut self,
        stage: &StageInfo<L>,
        events: &[MutationEvent],
    ) {
        // Move destinations found to stay under the root, so there is one
        // ancestor walk per destination. An entry never goes stale: a
        // destination can only leave the root if an ancestor moves out, and
        // that move's own check sets `move_occurred`.
        let mut inside_root: HashSet<StatementParent> = HashSet::new();
        for event in events {
            match *event {
                MutationEvent::CreatedStatement { stmt } => {
                    self.adopt(stage, stmt);
                    self.push_neighbors(stage, stmt);
                }
                MutationEvent::PlacedStatement { stmt }
                    if self.status.get(stmt) == Status::Outside =>
                {
                    self.adopt(stage, stmt);
                    self.push_neighbors(stage, stmt);
                }
                MutationEvent::ChangedOperands { stmt }
                | MutationEvent::ReplacedStatement { stmt } => self.push_neighbors(stage, stmt),
                MutationEvent::PlacedStatement { stmt } => {
                    // Already set: the sweep checks each statement it takes off the
                    // worklist against the root.
                    if self.move_occurred {
                        continue;
                    }
                    // Erased since it moved: the sweep skips it when it comes up.
                    if stmt.get_info(stage).is_none() {
                        continue;
                    }
                    // The block or graph it was moved into.
                    let Some(dest) = *stmt.parent(stage) else {
                        continue;
                    };
                    // Checked already in this call, and is under the root.
                    if inside_root.contains(&dest) {
                        continue;
                    }
                    // One ancestor walk: remember the destination if it is under
                    // the root, otherwise statements may have left the root.
                    if self.root.is_some_and(|root| stmt.is_within(stage, root)) {
                        inside_root.insert(dest);
                    } else {
                        self.move_occurred = true;
                    }
                }
                MutationEvent::SplicedCfg { dest, .. } => {
                    let owner = dest.get_info(stage).and_then(|info| info.parent());
                    let inside = owner
                        .zip(self.root)
                        .is_some_and(|(owner, root)| owner.is_within(stage, root));
                    if !inside {
                        self.move_occurred = true;
                    }
                }
                _ => {}
            }
        }
    }

    /// Count the inserted `stmt` under the root if its owner is.
    fn adopt<L: Dialect>(&mut self, stage: &StageInfo<L>, stmt: Statement) {
        if let Some(owner) = stmt.owner(stage)
            && self.status.get(owner) != Status::Outside
            && self.status.get(stmt) == Status::Outside
        {
            self.status.set(stmt, Status::VisitedOrInserted);
            self.inserted.push(stmt);
        }
    }

    /// Add `stmt`'s owner, the producers of its operands, `stmt` itself and
    /// the users of its results, top-down, as the walk visits them. Nothing
    /// if `stmt` is not live.
    ///
    /// The owner and the producers are added only if native rules are
    /// present. A pattern matches down from its root into producers, never
    /// into users or bodies, so an edit gives neither a new pattern match.
    /// The owner is never added if it is the root: visiting the root again
    /// would run its block rules over its whole body again, and the next
    /// sweep starts at the root anyway.
    fn push_neighbors<L: Dialect>(&mut self, stage: &StageInfo<L>, stmt: Statement) {
        let Some(info) = stmt.get_info(stage) else {
            return;
        };
        let definition = info.definition();
        if self.native_rules_present {
            if let Some(owner) = stmt.owner(stage)
                && Some(owner) != self.root
            {
                self.push(owner);
            }
            for &operand in definition.arguments() {
                if let Some(SSAKind::Result(producer, _)) =
                    operand.get_info(stage).map(|v| *v.kind())
                {
                    self.push(producer);
                }
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
