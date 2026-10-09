//! Blocker-cycle detection, kept pure so it can be tested without a database.

use std::collections::{HashMap, HashSet};

/// Would giving `id` the blocker list `new` make `id` (transitively) block itself?
/// `blockers_of` holds the current blocker list of every Todo.
pub(crate) fn creates_cycle(blockers_of: &HashMap<u32, Vec<u32>>, id: u32, new: &[u32]) -> bool {
    let mut seen = HashSet::new();
    let mut stack = new.to_vec();
    while let Some(next) = stack.pop() {
        if next == id {
            return true;
        }
        if seen.insert(next) {
            stack.extend(blockers_of.get(&next).into_iter().flatten().copied());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(edges: &[(u32, &[u32])]) -> HashMap<u32, Vec<u32>> {
        edges.iter().map(|(id, b)| (*id, b.to_vec())).collect()
    }

    #[test]
    fn t4_self_block_is_a_cycle() {
        assert!(creates_cycle(&graph(&[]), 1, &[1]));
    }

    #[test]
    fn t4_direct_back_edge_is_a_cycle() {
        assert!(creates_cycle(&graph(&[(2, &[1])]), 1, &[2]));
    }

    #[test]
    fn t4_transitive_back_edge_is_a_cycle() {
        assert!(creates_cycle(&graph(&[(2, &[1]), (3, &[2])]), 1, &[3]));
    }

    #[test]
    fn t4_diamond_is_not_a_cycle() {
        let g = graph(&[(2, &[1]), (3, &[1]), (4, &[2, 3])]);
        assert!(!creates_cycle(&g, 5, &[4, 1]));
    }

    #[test]
    fn t4_existing_cycle_elsewhere_does_not_loop_forever() {
        let g = graph(&[(2, &[3]), (3, &[2])]);
        assert!(!creates_cycle(&g, 1, &[2]));
    }
}
