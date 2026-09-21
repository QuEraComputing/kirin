use std::collections::HashMap;
use std::hash::Hash;

/// Order-insensitive, multiplicity-preserving equality.
///
/// Mirrors must not be compared with `==` on their storage. Our rewrite
/// functions may reorder lists, so an accurate mirror and a fresh derivation
/// routinely hold the same entries in different orders.
///
/// Multiplicity is kept rather than collapsing to a set: a statement can read
/// one value twice (`add %x, %x` is two distinct uses of `%x`), so losing a
/// duplicate is a real defect that a set comparison would hide.
pub(super) fn multiset_eq<T: Copy + Eq + Hash>(a: &[T], b: &[T]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut counts: HashMap<T, usize> = HashMap::with_capacity(a.len());
    for entry in a {
        *counts.entry(*entry).or_default() += 1;
    }
    for entry in b {
        match counts.get_mut(entry) {
            Some(count) => {
                *count -= 1;
                if *count == 0 {
                    counts.remove(entry);
                }
            }
            None => return false,
        }
    }
    counts.is_empty()
}

#[cfg(test)]
mod tests {
    use super::multiset_eq;

    #[test]
    fn ignores_order_but_not_multiplicity() {
        assert!(multiset_eq(&[1, 2, 3], &[3, 1, 2]));
        assert!(multiset_eq::<u8>(&[], &[]));
        // Duplicates are significant: `add %x, %x` is two uses of `%x`.
        assert!(multiset_eq(&[1, 1, 2], &[1, 2, 1]));
        assert!(!multiset_eq(&[1, 1, 2], &[1, 2, 2]));
        assert!(!multiset_eq(&[1, 2], &[1, 2, 2]));
    }
}
