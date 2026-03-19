use kirin_ir::{HasBottom, HasTop, Lattice};

use super::BoolInterval;
use crate::Interval;

// --- From<BoolInterval> for Interval ---

#[test]
fn true_converts_to_one() {
    assert_eq!(Interval::from(BoolInterval::True), Interval::constant(1));
}

#[test]
fn false_converts_to_zero() {
    assert_eq!(Interval::from(BoolInterval::False), Interval::constant(0));
}

#[test]
fn unknown_converts_to_zero_one() {
    assert_eq!(Interval::from(BoolInterval::Unknown), Interval::new(0, 1));
}

#[test]
fn bottom_converts_to_empty() {
    assert!(Interval::from(BoolInterval::Bottom).is_empty());
}

// --- Lattice: join ---

#[test]
fn join_bottom_identity() {
    assert_eq!(
        BoolInterval::Bottom.join(&BoolInterval::True),
        BoolInterval::True
    );
    assert_eq!(
        BoolInterval::True.join(&BoolInterval::Bottom),
        BoolInterval::True
    );
    assert_eq!(
        BoolInterval::Bottom.join(&BoolInterval::False),
        BoolInterval::False
    );
    assert_eq!(
        BoolInterval::Bottom.join(&BoolInterval::Unknown),
        BoolInterval::Unknown
    );
    assert_eq!(
        BoolInterval::Bottom.join(&BoolInterval::Bottom),
        BoolInterval::Bottom
    );
}

#[test]
fn join_same_values() {
    assert_eq!(
        BoolInterval::True.join(&BoolInterval::True),
        BoolInterval::True
    );
    assert_eq!(
        BoolInterval::False.join(&BoolInterval::False),
        BoolInterval::False
    );
    assert_eq!(
        BoolInterval::Unknown.join(&BoolInterval::Unknown),
        BoolInterval::Unknown
    );
}

#[test]
fn join_different_concretes_gives_unknown() {
    assert_eq!(
        BoolInterval::True.join(&BoolInterval::False),
        BoolInterval::Unknown
    );
    assert_eq!(
        BoolInterval::False.join(&BoolInterval::True),
        BoolInterval::Unknown
    );
}

#[test]
fn join_unknown_absorbs() {
    assert_eq!(
        BoolInterval::Unknown.join(&BoolInterval::True),
        BoolInterval::Unknown
    );
    assert_eq!(
        BoolInterval::Unknown.join(&BoolInterval::False),
        BoolInterval::Unknown
    );
    assert_eq!(
        BoolInterval::True.join(&BoolInterval::Unknown),
        BoolInterval::Unknown
    );
    assert_eq!(
        BoolInterval::False.join(&BoolInterval::Unknown),
        BoolInterval::Unknown
    );
}

// --- Lattice: meet ---

#[test]
fn meet_unknown_identity() {
    assert_eq!(
        BoolInterval::Unknown.meet(&BoolInterval::True),
        BoolInterval::True
    );
    assert_eq!(
        BoolInterval::True.meet(&BoolInterval::Unknown),
        BoolInterval::True
    );
    assert_eq!(
        BoolInterval::Unknown.meet(&BoolInterval::False),
        BoolInterval::False
    );
    assert_eq!(
        BoolInterval::Unknown.meet(&BoolInterval::Unknown),
        BoolInterval::Unknown
    );
}

#[test]
fn meet_same_values() {
    assert_eq!(
        BoolInterval::True.meet(&BoolInterval::True),
        BoolInterval::True
    );
    assert_eq!(
        BoolInterval::False.meet(&BoolInterval::False),
        BoolInterval::False
    );
}

#[test]
fn meet_different_concretes_gives_bottom() {
    assert_eq!(
        BoolInterval::True.meet(&BoolInterval::False),
        BoolInterval::Bottom
    );
    assert_eq!(
        BoolInterval::False.meet(&BoolInterval::True),
        BoolInterval::Bottom
    );
}

#[test]
fn meet_bottom_absorbs() {
    assert_eq!(
        BoolInterval::Bottom.meet(&BoolInterval::True),
        BoolInterval::Bottom
    );
    assert_eq!(
        BoolInterval::Bottom.meet(&BoolInterval::False),
        BoolInterval::Bottom
    );
    assert_eq!(
        BoolInterval::True.meet(&BoolInterval::Bottom),
        BoolInterval::Bottom
    );
}

// --- Lattice: is_subseteq ---

#[test]
fn bottom_subseteq_everything() {
    assert!(BoolInterval::Bottom.is_subseteq(&BoolInterval::Bottom));
    assert!(BoolInterval::Bottom.is_subseteq(&BoolInterval::True));
    assert!(BoolInterval::Bottom.is_subseteq(&BoolInterval::False));
    assert!(BoolInterval::Bottom.is_subseteq(&BoolInterval::Unknown));
}

#[test]
fn everything_subseteq_unknown() {
    assert!(BoolInterval::True.is_subseteq(&BoolInterval::Unknown));
    assert!(BoolInterval::False.is_subseteq(&BoolInterval::Unknown));
    assert!(BoolInterval::Unknown.is_subseteq(&BoolInterval::Unknown));
}

#[test]
fn concrete_not_subseteq_other_concrete() {
    assert!(!BoolInterval::True.is_subseteq(&BoolInterval::False));
    assert!(!BoolInterval::False.is_subseteq(&BoolInterval::True));
}

#[test]
fn unknown_not_subseteq_concrete() {
    assert!(!BoolInterval::Unknown.is_subseteq(&BoolInterval::True));
    assert!(!BoolInterval::Unknown.is_subseteq(&BoolInterval::False));
}

// --- HasBottom / HasTop ---

#[test]
fn has_bottom() {
    assert_eq!(BoolInterval::bottom(), BoolInterval::Bottom);
}

#[test]
fn has_top() {
    assert_eq!(BoolInterval::top(), BoolInterval::Unknown);
}

// --- Lattice laws ---

#[test]
fn bottom_join_identity_law() {
    let all = [
        BoolInterval::Bottom,
        BoolInterval::True,
        BoolInterval::False,
        BoolInterval::Unknown,
    ];
    for x in all {
        assert_eq!(
            BoolInterval::bottom().join(&x),
            x,
            "bottom.join({x:?}) should be {x:?}"
        );
    }
}

#[test]
fn top_meet_identity_law() {
    let all = [
        BoolInterval::Bottom,
        BoolInterval::True,
        BoolInterval::False,
        BoolInterval::Unknown,
    ];
    for x in all {
        assert_eq!(
            BoolInterval::top().meet(&x),
            x,
            "top.meet({x:?}) should be {x:?}"
        );
    }
}

#[test]
fn subseteq_consistent_with_join() {
    let all = [
        BoolInterval::Bottom,
        BoolInterval::True,
        BoolInterval::False,
        BoolInterval::Unknown,
    ];
    for a in all {
        for b in all {
            assert_eq!(
                a.is_subseteq(&b),
                a.join(&b) == b,
                "subseteq({a:?}, {b:?}) should match join({a:?}, {b:?}) == {b:?}"
            );
        }
    }
}
