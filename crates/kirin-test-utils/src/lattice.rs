//! Assertion helpers for verifying lattice algebraic laws.
//!
//! These check properties over a given set of sample elements and collect all
//! violations into a single report, so you can see every failing law at once
//! rather than fixing them one at a time.
//!
//! # Example
//!
//! ```
//! use kirin_test_utils::lattice::assert_finite_lattice_laws;
//! use kirin_test_utils::UnitType;
//!
//! // Pass representative elements from your lattice.
//! // Bottom and top are tested automatically.
//! assert_finite_lattice_laws(&[UnitType]);
//! ```

use kirin_ir::{HasBottom, HasTop, Lattice};
use std::fmt::{Debug, Write};

/// Collect violations into a `Vec<String>`, then panic with a combined report
/// if any were found.
fn report(violations: Vec<String>) {
    if violations.is_empty() {
        return;
    }
    let mut msg = format!("{} lattice law violation(s):\n", violations.len());
    for (i, v) in violations.iter().enumerate() {
        let _ = write!(msg, "  {}. {}\n", i + 1, v);
    }
    panic!("{msg}");
}

/// Check that `join` is commutative, associative, and idempotent over the
/// given elements.
///
/// Specifically, for every pair `(a, b)` and triple `(a, b, c)` drawn from
/// `elements`, this verifies:
/// - **Commutative**: `a.join(&b) == b.join(&a)`
/// - **Associative**: `a.join(&b).join(&c) == a.join(&b.join(&c))`
/// - **Idempotent**: `a.join(&a) == a`
///
/// # Example
///
/// ```
/// use kirin_test_utils::lattice::assert_join_laws;
/// use kirin_test_utils::UnitType;
///
/// assert_join_laws(&[UnitType]);
/// ```
pub fn assert_join_laws<L: Lattice + PartialEq + Debug>(elements: &[L]) {
    let mut violations = Vec::new();
    check_join_laws(elements, &mut violations);
    report(violations);
}

/// Check that `meet` is commutative, associative, and idempotent over the
/// given elements.
///
/// Specifically, for every pair `(a, b)` and triple `(a, b, c)` drawn from
/// `elements`, this verifies:
/// - **Commutative**: `a.meet(&b) == b.meet(&a)`
/// - **Associative**: `a.meet(&b).meet(&c) == a.meet(&b.meet(&c))`
/// - **Idempotent**: `a.meet(&a) == a`
///
/// # Example
///
/// ```
/// use kirin_test_utils::lattice::assert_meet_laws;
/// use kirin_test_utils::UnitType;
///
/// assert_meet_laws(&[UnitType]);
/// ```
pub fn assert_meet_laws<L: Lattice + PartialEq + Debug>(elements: &[L]) {
    let mut violations = Vec::new();
    check_meet_laws(elements, &mut violations);
    report(violations);
}

/// Check the absorption laws over the given elements.
///
/// For every pair `(a, b)` drawn from `elements`, this verifies:
/// - `a.join(&a.meet(&b)) == a`
/// - `a.meet(&a.join(&b)) == a`
///
/// # Example
///
/// ```
/// use kirin_test_utils::lattice::assert_absorption;
/// use kirin_test_utils::UnitType;
///
/// assert_absorption(&[UnitType]);
/// ```
pub fn assert_absorption<L: Lattice + PartialEq + Debug>(elements: &[L]) {
    let mut violations = Vec::new();
    check_absorption(elements, &mut violations);
    report(violations);
}

/// Check that `is_subseteq` is consistent with `join` and `meet` over the
/// given elements.
///
/// For every pair `(a, b)` drawn from `elements`, this verifies:
/// - `a.is_subseteq(&b)` if and only if `a.join(&b) == b`
/// - `a.is_subseteq(&b)` if and only if `a.meet(&b) == a`
///
/// # Example
///
/// ```
/// use kirin_test_utils::lattice::assert_ordering_consistent;
/// use kirin_test_utils::UnitType;
///
/// assert_ordering_consistent(&[UnitType]);
/// ```
pub fn assert_ordering_consistent<L: Lattice + PartialEq + Debug>(elements: &[L]) {
    let mut violations = Vec::new();
    check_ordering_consistent(elements, &mut violations);
    report(violations);
}

/// Check all lattice laws over the given elements: join laws, meet laws,
/// absorption, and ordering consistency. All violations are collected and
/// reported together.
///
/// This is the main entry point for testing a [`Lattice`] implementation. Pass
/// a representative set of elements — the more diverse the set, the better the
/// coverage. For lattices that also implement [`HasBottom`] and [`HasTop`], use
/// [`assert_finite_lattice_laws`] instead.
///
/// # Example
///
/// ```
/// use kirin_test_utils::lattice::assert_lattice_laws;
/// use kirin_test_utils::UnitType;
///
/// assert_lattice_laws(&[UnitType]);
/// ```
pub fn assert_lattice_laws<L: Lattice + PartialEq + Debug>(elements: &[L]) {
    let mut violations = Vec::new();
    check_join_laws(elements, &mut violations);
    check_meet_laws(elements, &mut violations);
    check_absorption(elements, &mut violations);
    check_ordering_consistent(elements, &mut violations);
    report(violations);
}

/// Check that `bottom()` satisfies the bottom element laws against every
/// element in the given slice.
///
/// For every element `x`, this verifies:
/// - `bottom().is_subseteq(&x)` (bottom is below everything)
/// - `bottom().join(&x) == x` (bottom is the identity for join)
/// - `bottom().meet(&x) == bottom()` (bottom absorbs meet)
///
/// # Example
///
/// ```
/// use kirin_test_utils::lattice::assert_bottom_laws;
/// use kirin_test_utils::UnitType;
///
/// assert_bottom_laws(&[UnitType]);
/// ```
pub fn assert_bottom_laws<L: HasBottom + PartialEq + Debug>(elements: &[L]) {
    let mut violations = Vec::new();
    check_bottom_laws(elements, &mut violations);
    report(violations);
}

/// Check that `top()` satisfies the top element laws against every element
/// in the given slice.
///
/// For every element `x`, this verifies:
/// - `x.is_subseteq(&top())` (everything is below top)
/// - `top().join(&x) == top()` (top absorbs join)
/// - `top().meet(&x) == x` (top is the identity for meet)
///
/// # Example
///
/// ```
/// use kirin_test_utils::lattice::assert_top_laws;
/// use kirin_test_utils::UnitType;
///
/// assert_top_laws(&[UnitType]);
/// ```
pub fn assert_top_laws<L: HasTop + PartialEq + Debug>(elements: &[L]) {
    let mut violations = Vec::new();
    check_top_laws(elements, &mut violations);
    report(violations);
}

/// Check all lattice laws plus bottom and top element laws. All violations are
/// collected and reported together.
///
/// This is the most comprehensive check for lattices that implement both
/// [`HasBottom`] and [`HasTop`]. The `elements` slice should include
/// representative values from your lattice; bottom and top are tested
/// automatically in addition to the provided elements.
///
/// # Example
///
/// ```
/// use kirin_test_utils::lattice::assert_finite_lattice_laws;
/// use kirin_test_utils::UnitType;
///
/// assert_finite_lattice_laws(&[UnitType]);
/// ```
pub fn assert_finite_lattice_laws<L: HasBottom + HasTop + PartialEq + Debug>(elements: &[L]) {
    let mut violations = Vec::new();
    check_join_laws(elements, &mut violations);
    check_meet_laws(elements, &mut violations);
    check_absorption(elements, &mut violations);
    check_ordering_consistent(elements, &mut violations);
    check_bottom_laws(elements, &mut violations);
    check_top_laws(elements, &mut violations);
    report(violations);
}

// ---- internal helpers that push violations instead of panicking ----

fn check_join_laws<L: Lattice + PartialEq + Debug>(elements: &[L], v: &mut Vec<String>) {
    for a in elements {
        // idempotent
        if a.join(a) != *a {
            v.push(format!("join not idempotent: {a:?}.join({a:?}) != {a:?}"));
        }
        for b in elements {
            // commutative
            if a.join(b) != b.join(a) {
                v.push(format!(
                    "join not commutative: {a:?}.join({b:?}) != {b:?}.join({a:?})"
                ));
            }
            // associative
            for c in elements {
                if a.join(b).join(c) != a.join(&b.join(c)) {
                    v.push(format!(
                        "join not associative: ({a:?}.join({b:?})).join({c:?}) != {a:?}.join({b:?}.join({c:?}))"
                    ));
                }
            }
        }
    }
}

fn check_meet_laws<L: Lattice + PartialEq + Debug>(elements: &[L], v: &mut Vec<String>) {
    for a in elements {
        // idempotent
        if a.meet(a) != *a {
            v.push(format!("meet not idempotent: {a:?}.meet({a:?}) != {a:?}"));
        }
        for b in elements {
            // commutative
            if a.meet(b) != b.meet(a) {
                v.push(format!(
                    "meet not commutative: {a:?}.meet({b:?}) != {b:?}.meet({a:?})"
                ));
            }
            // associative
            for c in elements {
                if a.meet(b).meet(c) != a.meet(&b.meet(c)) {
                    v.push(format!(
                        "meet not associative: ({a:?}.meet({b:?})).meet({c:?}) != {a:?}.meet({b:?}.meet({c:?}))"
                    ));
                }
            }
        }
    }
}

fn check_absorption<L: Lattice + PartialEq + Debug>(elements: &[L], v: &mut Vec<String>) {
    for a in elements {
        for b in elements {
            if a.join(&a.meet(b)) != *a {
                v.push(format!(
                    "absorption violated: {a:?}.join({a:?}.meet({b:?})) != {a:?}"
                ));
            }
            if a.meet(&a.join(b)) != *a {
                v.push(format!(
                    "absorption violated: {a:?}.meet({a:?}.join({b:?})) != {a:?}"
                ));
            }
        }
    }
}

fn check_ordering_consistent<L: Lattice + PartialEq + Debug>(elements: &[L], v: &mut Vec<String>) {
    for a in elements {
        for b in elements {
            let sub = a.is_subseteq(b);
            let join_agrees = a.join(b) == *b;
            let meet_agrees = a.meet(b) == *a;
            if sub != join_agrees {
                v.push(format!(
                    "ordering inconsistent with join: {a:?}.is_subseteq({b:?}) = {sub}, \
                     but {a:?}.join({b:?}) == {b:?} is {join_agrees}"
                ));
            }
            if sub != meet_agrees {
                v.push(format!(
                    "ordering inconsistent with meet: {a:?}.is_subseteq({b:?}) = {sub}, \
                     but {a:?}.meet({b:?}) == {a:?} is {meet_agrees}"
                ));
            }
        }
    }
}

fn check_bottom_laws<L: HasBottom + PartialEq + Debug>(elements: &[L], v: &mut Vec<String>) {
    let bot = L::bottom();
    for x in elements {
        if !bot.is_subseteq(x) {
            v.push(format!(
                "bottom not below element: bottom().is_subseteq({x:?}) = false"
            ));
        }
        if bot.join(x) != *x {
            v.push(format!(
                "bottom identity violated: bottom().join({x:?}) != {x:?}"
            ));
        }
        if bot.meet(x) != bot {
            v.push(format!(
                "bottom annihilation violated: bottom().meet({x:?}) != bottom()"
            ));
        }
    }
}

fn check_top_laws<L: HasTop + PartialEq + Debug>(elements: &[L], v: &mut Vec<String>) {
    let top = L::top();
    for x in elements {
        if !x.is_subseteq(&top) {
            v.push(format!(
                "element not below top: {x:?}.is_subseteq(top()) = false"
            ));
        }
        if top.join(x) != top {
            v.push(format!(
                "top annihilation violated: top().join({x:?}) != top()"
            ));
        }
        if top.meet(x) != *x {
            v.push(format!("top identity violated: top().meet({x:?}) != {x:?}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UnitType;

    #[test]
    fn unit_type_lattice_laws() {
        assert_finite_lattice_laws(&[UnitType]);
    }
}
