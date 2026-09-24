//! Reconstructs a chain of Statements in a Block or Blocks in a CFG.
//! The chains are constructed from the links derived from the authoritative IR.
//!
//! A container's `head`/`tail`/`len` is a mirror; the truth is the `prev`/
//! `next` pair stored on each element plus its parent pointer. This module
//! rebuilds the former from the latter, reporting every way the links can fail
//! to describe one well-formed chain.

#![allow(dead_code)]

use std::{collections::HashSet, fmt};

use smallvec::SmallVec;

use crate::{Block, CFG, Identifier, LinkedList, Statement, derived::SlotMap};

/// One chain defect, over a single element type `I`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChainDefect<I: Identifier> {
    /// Walking the chain reached the same element twice
    Cycle(I),
    /// There is more that one member in the container that begins a chain.
    MultipleStarts(SmallVec<[I; 2]>),
    /// `from.next == to` but `to.prev != from`.
    BadLink { from: I, to: I },
    /// Members that belong to the container but the chain never reaches.
    Orphans(Vec<I>),
    /// `from.next` names `to`, which is not live.
    DanglingLink { from: I, to: I },
    /// `from.next` names `to`, which has a different parent.
    CrossParent { from: I, to: I },
}

impl<I: Identifier> fmt::Display for ChainDefect<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainDefect::Cycle(at) => write!(f, "the chain revisits {at:?}"),
            ChainDefect::MultipleStarts(starts) => write!(
                f,
                "{} members have no predecessor, so they begin disjoint chains: {starts:?}",
                starts.len()
            ),
            ChainDefect::BadLink { from, to } => write!(
                f,
                "{from:?} links forward to {to:?}, but {to:?} does not link back to it"
            ),
            ChainDefect::Orphans(members) => write!(
                f,
                "{} members are never reached by the chain: {members:?}",
                members.len()
            ),
            ChainDefect::DanglingLink { from, to } => write!(
                f,
                "the next link of {from:?} names {to:?}, which is not live"
            ),
            ChainDefect::CrossParent { from, to } => write!(
                f,
                "the next link of {from:?} names {to:?}, which has a different parent"
            ),
        }
    }
}

/// Which chain a defect came from. The element type is fixed per variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChainFinding {
    StatementsInBlock(ChainDefect<Statement>, Block),
    BlocksInCFG(ChainDefect<Block>, CFG),
}

impl fmt::Display for ChainFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainFinding::StatementsInBlock(defect, block) => {
                write!(f, "defect in {block:?}'s chain: {defect}")
            }
            ChainFinding::BlocksInCFG(defect, cfg) => {
                write!(f, "defect in {cfg:?}'s chain: {defect}")
            }
        }
    }
}

/// The authoritative link data for one element, extracted from the IR.
/// An element may be a Statement in a Block or a Block in a CFG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ChainLink<I: Identifier, C: Identifier> {
    prev: Option<I>,
    next: Option<I>,
    parent: C,
}

/// The information extracted from [`StageInfo`](crate::StageInfo)'s arenas
/// needed to derive the chains.
///
/// `links` holds every live element's authoritative links; an absent entry
/// means the element is tombstoned or out of range, which is what separates a
/// dangling link from one that merely crosses into another container.
/// `members` groups those elements by container, in arena order.
///
/// A [`ChainScan`] is built by each mirror which shall then call `derive_chains`.
pub(super) struct ChainScan<I: Identifier, C: Identifier> {
    links: SlotMap<I, Option<ChainLink<I, C>>>,
    members: SlotMap<C, Vec<I>>,
}

impl<I: Identifier, C: Identifier> ChainScan<I, C> {
    /// An empty scan sized to hold every slot of both arenas, tombstones
    /// included, so an id indexes each map directly.
    pub(super) fn new(element_slots: usize, container_slots: usize) -> Self {
        Self {
            links: SlotMap::with_len(element_slots),
            members: SlotMap::with_len(container_slots),
        }
    }

    /// Record one live element: its links, and its membership of `parent`.
    pub(super) fn record(&mut self, element: I, prev: Option<I>, next: Option<I>, parent: C) {
        if let Some(slot) = self.links.get_mut(element) {
            *slot = Some(ChainLink { prev, next, parent });
        }
        if let Some(members) = self.members.get_mut(parent) {
            members.push(element);
        }
    }
}

/// Reconstruct the `head`/`tail`/`len` summary of every container's chain.
///
/// Pure: nothing is written back. Each defective container (Block or CFG)
/// contributes its defects to `defects` and maps to `None`; sound containers
/// are unaffected, so one call reports every independently broken chain in the stage.
pub(super) fn derive_chains<I: Identifier, C: Identifier>(
    scan: &ChainScan<I, C>,
    defects: &mut Vec<(C, ChainDefect<I>)>,
) -> SlotMap<C, Option<LinkedList<I>>> {
    let mut chains = SlotMap::with_len(scan.members.len());

    for (container, members) in scan.members.iter() {
        let mut found = Vec::new();
        let chain = derive_chain(members, &scan.links, &mut found);
        defects.extend(found.into_iter().map(|defect| (container, defect)));

        if let Some(slot) = chains.get_mut(container) {
            *slot = chain;
        }
    }

    chains
}

/// Reconstruct one container's chain summary from its members' links.
///
/// Returns `None` when the links do not describe one well-formed chain, having
/// reported why in `defects`.
///
/// `members`' Ids must always be in range of `links` domain.
///
/// Walking stops at the first link that cannot be trusted, because there is no
/// sound way to continue from it. Coverage is still checked, so a truncated
/// walk also reports what it stranded.
fn derive_chain<I: Identifier, C: Identifier>(
    members: &[I],
    links: &SlotMap<I, Option<ChainLink<I, C>>>,
    defects: &mut Vec<ChainDefect<I>>,
) -> Option<LinkedList<I>> {
    // Block only contains a terminator => Valid empty chain
    if members.is_empty() {
        return Some(LinkedList::new());
    }

    let before = defects.len();
    let link_of = |element: I| links.get(element).copied().flatten();

    // The chain starts at the one member with no predecessor. Never consult
    // the installed `head`: that is the mirror being derived.
    let starts: SmallVec<[I; 2]> = members
        .iter()
        .copied()
        .filter(|member| link_of(*member).is_some_and(|link| link.prev.is_none()))
        .collect();

    match starts.len() {
        // Every member has a predecessor, so the members form one or more
        // closed cycles and the chain has no entry point.
        0 => {
            defects.push(ChainDefect::Cycle(members[0]));
            return None;
        }
        1 => {}
        // Several members begin chains: the container is disconnected.
        _ => {
            defects.push(ChainDefect::MultipleStarts(starts));
            return None;
        }
    }

    let mut visited = HashSet::new();
    let mut current = starts[0];

    // Bounded by the member count
    loop {
        visited.insert(current);

        // Always present: `starts` was filtered on having a link, and the walk
        // only advances to elements whose link it has already read.
        let forward = link_of(current).unwrap();

        // Reached the end of the chain, so `current` is the tail.
        let Some(next) = forward.next else {
            break;
        };

        // An absent entry means `next` is tombstoned or out of range; either
        // way the link names something that is not a live element.
        let Some(forward_next) = link_of(next) else {
            defects.push(ChainDefect::DanglingLink {
                from: current,
                to: next,
            });
            break;
        };

        if forward_next.parent != forward.parent {
            defects.push(ChainDefect::CrossParent {
                from: current,
                to: next,
            });
            break;
        }

        if forward_next.prev != Some(current) {
            defects.push(ChainDefect::BadLink {
                from: current,
                to: next,
            });
            break;
        }

        if visited.contains(&next) {
            defects.push(ChainDefect::Cycle(next));
            break;
        }

        current = next;
    }

    // Membership is authoritative, so anything the walk missed is a defect
    // even when every link it did cross was sound.
    let orphans = unvisited(members, &visited);
    if !orphans.is_empty() {
        defects.push(ChainDefect::Orphans(orphans));
    }

    // Return `None` after having encountered any defects
    if defects.len() != before {
        return None;
    }

    Some(LinkedList {
        head: Some(starts[0]),
        tail: Some(current),
        len: visited.len(),
    })
}

/// The members the walk never reached, in `members` order.
fn unvisited<I: Identifier>(members: &[I], visited: &HashSet<I>) -> Vec<I> {
    members
        .iter()
        .copied()
        .filter(|member| !visited.contains(member))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Id;

    /// These tests build a [`ChainScan`] directly rather than going through a
    /// [`StageInfo`](crate::StageInfo).
    fn stmt(raw: usize) -> Statement {
        Statement::from(Id(raw))
    }

    fn block(raw: usize) -> Block {
        Block::from(Id(raw))
    }

    /// A scan over `slots` statements and 2 blocks.
    fn scan(slots: usize) -> ChainScan<Statement, Block> {
        ChainScan::new(slots, 2)
    }

    /// Record `members` as a well-formed chain in `parent`, in order.
    fn link_chain(scan: &mut ChainScan<Statement, Block>, parent: Block, members: &[Statement]) {
        for (index, &member) in members.iter().enumerate() {
            scan.record(
                member,
                index.checked_sub(1).map(|prev| members[prev]),
                members.get(index + 1).copied(),
                parent,
            );
        }
    }

    fn derive(
        scan: &ChainScan<Statement, Block>,
        parent: Block,
    ) -> (Option<LinkedList<Statement>>, Vec<ChainDefect<Statement>>) {
        let mut defects = Vec::new();
        let members = scan.members.get(parent).cloned().unwrap_or_default();
        let chain = derive_chain(&members, &scan.links, &mut defects);
        (chain, defects)
    }

    #[test]
    fn well_formed_chain_yields_head_tail_and_len() {
        let mut scan = scan(3);
        link_chain(&mut scan, block(0), &[stmt(0), stmt(1), stmt(2)]);

        let (chain, defects) = derive(&scan, block(0));
        assert!(defects.is_empty(), "{defects:?}");
        let chain = chain.expect("a sound chain derives");
        assert_eq!(chain.head(), Some(&stmt(0)));
        assert_eq!(chain.tail(), Some(&stmt(2)));
        assert_eq!(chain.len(), 3);
    }

    #[test]
    fn single_member_chain_is_its_own_head_and_tail() {
        let mut scan = scan(1);
        link_chain(&mut scan, block(0), &[stmt(0)]);

        let (chain, defects) = derive(&scan, block(0));
        assert!(defects.is_empty(), "{defects:?}");
        let chain = chain.expect("a sound chain derives");
        assert_eq!(chain.head(), Some(&stmt(0)));
        assert_eq!(chain.tail(), Some(&stmt(0)));
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn a_closed_cycle_has_no_start() {
        let mut scan = scan(2);
        // 0 <-> 1, reciprocal in both directions: no member has `prev == None`.
        scan.record(stmt(0), Some(stmt(1)), Some(stmt(1)), block(0));
        scan.record(stmt(1), Some(stmt(0)), Some(stmt(0)), block(0));

        let (chain, defects) = derive(&scan, block(0));
        assert_eq!(defects, vec![ChainDefect::Cycle(stmt(0))]);
        assert!(chain.is_none(), "a defective chain has no summary");
    }

    #[test]
    fn two_disjoint_chains_report_both_starts() {
        let mut scan = scan(4);
        link_chain(&mut scan, block(0), &[stmt(0), stmt(1)]);
        // A second chain wrongly parented into the same block.
        scan.record(stmt(2), None, Some(stmt(3)), block(0));
        scan.record(stmt(3), Some(stmt(2)), None, block(0));

        let (chain, defects) = derive(&scan, block(0));
        assert_eq!(defects.len(), 1);
        let ChainDefect::MultipleStarts(starts) = &defects[0] else {
            panic!("expected MultipleStarts, got {:?}", defects[0]);
        };
        assert_eq!(starts.as_slice(), &[stmt(0), stmt(2)]);
        assert!(chain.is_none(), "a defective chain has no summary");
    }

    #[test]
    fn a_link_to_a_tombstoned_element_is_dangling() {
        let mut scan = scan(2);
        // `stmt(1)` is never recorded, so it has no link entry: tombstoned or
        // out of range, which the walk cannot tell apart and need not.
        scan.record(stmt(0), None, Some(stmt(1)), block(0));

        let (chain, defects) = derive(&scan, block(0));
        assert_eq!(
            defects,
            vec![ChainDefect::DanglingLink {
                from: stmt(0),
                to: stmt(1),
            }]
        );
        assert!(chain.is_none(), "a defective chain has no summary");
    }

    #[test]
    fn a_link_into_another_container_is_cross_parent() {
        let mut scan = scan(2);
        scan.record(stmt(0), None, Some(stmt(1)), block(0));
        // Live and reciprocal, but parented elsewhere.
        scan.record(stmt(1), Some(stmt(0)), None, block(1));

        let (chain, defects) = derive(&scan, block(0));
        assert_eq!(
            defects,
            vec![ChainDefect::CrossParent {
                from: stmt(0),
                to: stmt(1),
            }]
        );
        assert!(chain.is_none(), "a defective chain has no summary");
    }

    #[test]
    fn a_non_reciprocal_link_is_a_bad_link() {
        let mut scan = scan(3);
        scan.record(stmt(0), None, Some(stmt(1)), block(0));
        // `stmt(1)` claims a predecessor other than `stmt(0)`, so it is neither
        // a chain start nor a valid successor.
        scan.record(stmt(1), Some(stmt(2)), None, block(0));

        let (chain, defects) = derive(&scan, block(0));
        assert_eq!(
            defects,
            vec![
                ChainDefect::BadLink {
                    from: stmt(0),
                    to: stmt(1),
                },
                // The walk stopped at the bad link, so `stmt(1)` really is
                // unreached; both facts are reported.
                ChainDefect::Orphans(vec![stmt(1)]),
            ]
        );
        assert!(chain.is_none(), "a defective chain has no summary");
    }

    #[test]
    fn members_the_walk_never_reaches_are_orphans() {
        let mut scan = scan(3);
        link_chain(&mut scan, block(0), &[stmt(0), stmt(1)]);
        // Parented into the block, but reachable from nothing: it claims a
        // predecessor that does not point back, so it is not a second start.
        scan.record(stmt(2), Some(stmt(0)), None, block(0));

        let (chain, defects) = derive(&scan, block(0));
        assert_eq!(defects, vec![ChainDefect::Orphans(vec![stmt(2)])]);
        assert!(chain.is_none(), "a defective chain has no summary");
    }

    #[test]
    fn a_sound_chain_reports_no_orphans() {
        let mut scan = scan(2);
        link_chain(&mut scan, block(0), &[stmt(0), stmt(1)]);

        let (_, defects) = derive(&scan, block(0));
        // An empty `Orphans` would be a defect with no content.
        assert!(defects.is_empty(), "{defects:?}");
    }

    #[test]
    fn a_truncated_walk_still_reports_what_it_stranded() {
        let mut scan = scan(3);
        // Dangles at the first step, so the walk stops immediately.
        scan.record(stmt(0), None, Some(stmt(1)), block(0));
        // Claims a predecessor, so it is not a second start — just unreachable.
        scan.record(stmt(2), Some(stmt(0)), None, block(0));

        let (_, defects) = derive(&scan, block(0));
        assert_eq!(
            defects,
            vec![
                ChainDefect::DanglingLink {
                    from: stmt(0),
                    to: stmt(1),
                },
                ChainDefect::Orphans(vec![stmt(2)]),
            ]
        );
    }

    #[test]
    fn derive_chains_walks_every_container_independently() {
        let mut scan = scan(3);
        link_chain(&mut scan, block(0), &[stmt(0), stmt(1)]);
        // A one-member chain whose `next` dangles.
        scan.record(stmt(2), None, Some(stmt(9)), block(1));

        let mut defects = Vec::new();
        let chains = derive_chains(&scan, &mut defects);

        // The sound container is unaffected by its neighbour's defect.
        let first = chains
            .get(block(0))
            .unwrap()
            .expect("a sound chain derives");
        assert_eq!(first.head(), Some(&stmt(0)));
        assert_eq!(first.len(), 2);

        assert!(chains.get(block(1)).unwrap().is_none());
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].0, block(1));
    }

    #[test]
    fn containers_with_no_members_are_empty_not_defective() {
        // A block holding nothing but a terminator has no chain members at all.
        let scan = scan(0);

        let mut defects = Vec::new();
        let chains = derive_chains(&scan, &mut defects);

        assert!(defects.is_empty());
        // `Some(empty)` rather than `None`: empty is a chain, not a failure.
        let empty = chains
            .get(block(0))
            .unwrap()
            .expect("empty is not a defect");
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.head(), None);
    }
}
