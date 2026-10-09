//! Field registers, the pure LWW merge, bounded history and the derived
//! concurrency view (docs/06 sections 5.2-5.4).
//!
//! Everything here is a pure function of its inputs: no storage, no clock.

use core::cmp::Ordering;

use arya_vault_storage::Id;
use zeroize::Zeroizing;

use crate::hlc::Hlc;

/// One version of one field: `(value, hlc, device_id, base_hlc)`.
///
/// `value == None` is a cleared field / tombstone. The encoded value is held in
/// a zeroizing buffer and never printed by `Debug`.
#[derive(Clone)]
pub struct Register {
    /// CBOR-encoded value, `None` = cleared.
    pub value: Option<Zeroizing<Vec<u8>>>,
    /// Timestamp of this version.
    pub hlc: Hlc,
    /// Authoring device.
    pub device_id: Id,
    /// `hlc` of the version the author saw when editing (`None` for the first).
    pub base_hlc: Option<Hlc>,
}

impl core::fmt::Debug for Register {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Register")
            .field("value_len", &self.value.as_ref().map(|v| v.len()))
            .field("hlc", &self.hlc)
            .field(
                "device",
                &self.device_id[..2]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
            )
            .field("base_hlc", &self.base_hlc)
            .finish()
    }
}

impl Register {
    /// Version identity: `(hlc, device_id)`.
    #[must_use]
    pub fn key(&self) -> (Hlc, Id) {
        (self.hlc, self.device_id)
    }

    /// The merge order. Doc 06 section 5.2 specifies `(hlc, device_id)`; two
    /// versions with the same key but different content can only come from a
    /// forked device id, and are ordered by `(value, base_hlc)` so that merge
    /// stays commutative even then (`None` sorts before `Some`).
    #[must_use]
    pub fn total_cmp(&self, other: &Self) -> Ordering {
        self.hlc
            .cmp(&other.hlc)
            .then_with(|| self.device_id.cmp(&other.device_id))
            .then_with(|| self.value.as_deref().cmp(&other.value.as_deref()))
            .then_with(|| self.base_hlc.cmp(&other.base_hlc))
    }

    /// Order used to decide which *history* entries to retain: newest `hlc`
    /// first, ties broken by ascending `device_id`. Fixed (and shared with the
    /// storage layer's pruning) so the retained set is independent of arrival order.
    #[must_use]
    pub fn retention_cmp(&self, other: &Self) -> Ordering {
        other
            .hlc
            .cmp(&self.hlc)
            .then_with(|| self.device_id.cmp(&other.device_id))
    }

    /// Merge two versions: the winner is the maximum under [`Register::total_cmp`]
    /// and the loser (to be stored in history) is returned unless both are identical.
    /// Commutative, associative (as a fold) and idempotent.
    #[must_use]
    pub fn merge(a: &Register, b: &Register) -> Merged {
        match a.total_cmp(b) {
            Ordering::Equal => Merged {
                winner: a.clone(),
                loser: None,
            },
            Ordering::Greater => Merged {
                winner: a.clone(),
                loser: Some(b.clone()),
            },
            Ordering::Less => Merged {
                winner: b.clone(),
                loser: Some(a.clone()),
            },
        }
    }
}

impl PartialEq for Register {
    fn eq(&self, other: &Self) -> bool {
        self.total_cmp(other) == Ordering::Equal
    }
}
impl Eq for Register {}

/// Result of [`Register::merge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// The version that becomes (or stays) current.
    pub winner: Register,
    /// The version that goes to history, if the inputs differ.
    pub loser: Option<Register>,
}

/// The stored state of one field: current version plus retained history.
///
/// `history` never contains the winner, holds at most one version per
/// `(hlc, device_id)` and is kept sorted by [`Register::retention_cmp`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldState {
    /// Current version.
    pub winner: Option<Register>,
    /// Older versions, newest first, at most `keep` of them.
    pub history: Vec<Register>,
}

impl FieldState {
    /// Apply one op (idempotent, order-independent). `keep` is the history limit.
    ///
    /// Final state depends only on the *set* of ops applied: the winner is the
    /// global maximum, and the history is the `keep` best (by retention order)
    /// of all other versions. A version pruned earlier always has `keep`
    /// better versions in history, and history members only ever leave by
    /// pruning, so no arrival order can resurrect or lose a retained version.
    pub fn apply(&mut self, op: Register, keep: usize) {
        match self.winner.take() {
            None => self.winner = Some(op),
            Some(w) => {
                let Merged { winner, loser } = Register::merge(&w, &op);
                self.winner = Some(winner);
                if let Some(l) = loser {
                    self.insert_history(l);
                }
            }
        }
        self.history.sort_by(Register::retention_cmp);
        self.history.truncate(keep);
    }

    fn insert_history(&mut self, v: Register) {
        match self.history.iter_mut().find(|h| h.key() == v.key()) {
            // Same (hlc, device) already retained: keep the greater, deterministically.
            Some(existing) => {
                if v.total_cmp(existing) == Ordering::Greater {
                    *existing = v;
                }
            }
            None => self.history.push(v),
        }
    }

    /// Versions that are not ancestors of the current one (see [`concurrent_losers`]).
    #[must_use]
    pub fn concurrent(&self) -> Vec<&Register> {
        match &self.winner {
            Some(w) => concurrent_losers(w, &self.history),
            None => Vec::new(),
        }
    }
}

/// The derived concurrency view (doc 06 section 5.3): the members of `history`
/// that are **not ancestors** of `winner`.
///
/// Ancestry follows `base_hlc` links: `winner.base_hlc` names the version it
/// was based on (matched by `hlc` among `history`), whose own `base_hlc` names
/// the next, and so on. A link is followed only if exactly one retained version
/// has that `hlc` and it is strictly older; if the ancestor was pruned (or the
/// `hlc` is ambiguous) the chain stops there and every older version is
/// conservatively reported as concurrent (shown, never hidden).
///
/// A pure function of `(winner, history)`, so identical on every replica
/// regardless of the order the ops arrived in. It never mutates state.
#[must_use]
pub fn concurrent_losers<'a>(winner: &Register, history: &'a [Register]) -> Vec<&'a Register> {
    let mut ancestors: Vec<usize> = Vec::new();
    let mut cur_hlc = winner.hlc;
    let mut next = winner.base_hlc;
    while let Some(b) = next {
        if b >= cur_hlc {
            break; // not causally earlier: ignore the link
        }
        let mut found = history.iter().enumerate().filter(|(_, h)| h.hlc == b);
        let (Some((i, hit)), None) = (found.next(), found.next()) else {
            break;
        };
        if ancestors.contains(&i) {
            break;
        }
        ancestors.push(i);
        cur_hlc = hit.hlc;
        next = hit.base_hlc;
    }
    let mut out: Vec<&Register> = history
        .iter()
        .enumerate()
        .filter(|(i, _)| !ancestors.contains(i))
        .map(|(_, h)| h)
        .collect();
    out.sort_by(|a, b| a.retention_cmp(b));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn h(pt: u64, c: u16) -> Hlc {
        Hlc::new(pt, c).unwrap()
    }
    fn reg(v: Option<&str>, hlc: Hlc, dev: u8, base: Option<Hlc>) -> Register {
        Register {
            value: v.map(|s| Zeroizing::new(s.as_bytes().to_vec())),
            hlc,
            device_id: [dev; 16],
            base_hlc: base,
        }
    }
    fn arb_register() -> impl Strategy<Value = Register> {
        // Tiny domains on purpose: collisions in (hlc, device) and in values are the hard cases.
        (
            0u64..6,
            0u16..2,
            0u8..3,
            proptest::option::of(0u8..3),
            proptest::option::of((0u64..6, 0u16..2)),
        )
            .prop_map(|(pt, c, dev, v, base)| Register {
                value: v.map(|b| Zeroizing::new(vec![b])),
                hlc: h(pt, c),
                device_id: [dev; 16],
                base_hlc: base.map(|(p, c)| h(p, c)),
            })
    }
    fn fold(ops: &[Register], keep: usize) -> FieldState {
        let mut s = FieldState::default();
        for o in ops {
            s.apply(o.clone(), keep);
        }
        s
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        // SEC-Y04: merge is commutative, associative and idempotent.
        #[test]
        fn merge_is_commutative(a in arb_register(), b in arb_register()) {
            prop_assert_eq!(Register::merge(&a, &b), Register::merge(&b, &a));
        }

        #[test]
        fn merge_is_associative(a in arb_register(), b in arb_register(), c in arb_register()) {
            let left = Register::merge(&Register::merge(&a, &b).winner, &c).winner;
            let right = Register::merge(&a, &Register::merge(&b, &c).winner).winner;
            prop_assert_eq!(left, right);
        }

        #[test]
        fn merge_is_idempotent(a in arb_register()) {
            let m = Register::merge(&a, &a);
            prop_assert_eq!(m.winner, a);
            prop_assert!(m.loser.is_none());
        }

        // SEC-Y04 / SEC-Y14: any permutation, with duplicates, gives the same state
        // and the same concurrency view.
        #[test]
        fn any_order_and_duplication_converges(
            ops in proptest::collection::vec(arb_register(), 1..14),
            keep in 1usize..8,
            seed in any::<u64>(),
            dups in proptest::collection::vec(any::<prop::sample::Index>(), 0..10),
        ) {
            let reference = fold(&ops, keep);
            let mut shuffled = ops.clone();
            for d in &dups { shuffled.push(ops[d.index(ops.len())].clone()); }
            // deterministic Fisher-Yates driven by `seed`
            let mut s = seed | 1;
            for i in (1..shuffled.len()).rev() {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                shuffled.swap(i, (s >> 33) as usize % (i + 1));
            }
            let other = fold(&shuffled, keep);
            prop_assert_eq!(&reference, &other);
            prop_assert_eq!(reference.concurrent(), other.concurrent());
            // Re-applying everything again changes nothing (idempotent).
            let mut again = other.clone();
            for o in &ops { again.apply(o.clone(), keep); }
            prop_assert_eq!(&again, &other);
        }

        #[test]
        fn winner_is_global_max_and_history_is_bounded_and_excludes_it(
            ops in proptest::collection::vec(arb_register(), 1..14), keep in 1usize..8,
        ) {
            let s = fold(&ops, keep);
            let max = ops.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
            prop_assert_eq!(s.winner.as_ref().unwrap(), max);
            prop_assert!(s.history.len() <= keep);
            let keys: Vec<_> = s.history.iter().map(Register::key).collect();
            let mut dedup = keys.clone(); dedup.sort(); dedup.dedup();
            prop_assert_eq!(keys.len(), dedup.len(), "one history entry per (hlc, device)");
            prop_assert!(s.history.windows(2).all(|w| w[0].retention_cmp(&w[1]) != Ordering::Greater));
        }
    }

    #[test]
    fn merge_returns_the_loser_for_history() {
        let a = reg(Some("a"), h(1, 0), 1, None);
        let b = reg(Some("b"), h(2, 0), 1, Some(h(1, 0)));
        let m = Register::merge(&a, &b);
        assert_eq!((m.winner.key(), m.loser.unwrap().key()), (b.key(), a.key()));
        // Same hlc: the larger device_id wins (doc 06 section 5.2).
        let c = reg(Some("c"), h(2, 0), 9, None);
        assert_eq!(Register::merge(&b, &c).winner.device_id, [9; 16]);
    }

    #[test]
    fn debug_never_prints_the_value() {
        let r = reg(Some("CANARY-SECRET"), h(1, 0), 1, None);
        assert!(!format!("{r:?}").contains("CANARY"));
    }

    /// Spurious-conflict regression (review H3): B is based on A. If B arrives
    /// first, A must not be reported as concurrent once A arrives.
    #[test]
    fn causally_later_edit_arriving_first_is_not_a_conflict() {
        let a = reg(Some("a"), h(10, 0), 1, None);
        let b = reg(Some("b"), h(20, 0), 2, Some(h(10, 0)));
        for order in [[&a, &b], [&b, &a]] {
            let mut s = FieldState::default();
            order.iter().for_each(|o| s.apply((*o).clone(), 5));
            assert_eq!(s.winner.as_ref().unwrap().key(), b.key());
            assert!(
                s.concurrent().is_empty(),
                "A is an ancestor of B in either arrival order"
            );
        }
    }

    #[test]
    fn genuinely_concurrent_edits_are_reported_in_every_order() {
        let a = reg(Some("a"), h(10, 0), 1, None);
        let b = reg(Some("b"), h(20, 0), 2, Some(h(10, 0)));
        let c = reg(Some("c"), h(21, 0), 3, Some(h(10, 0))); // also based on A: concurrent with B
        let ops = [a, b.clone(), c.clone()];
        let perms = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for p in perms {
            let mut s = FieldState::default();
            p.iter().for_each(|&i| s.apply(ops[i].clone(), 5));
            assert_eq!(s.winner.as_ref().unwrap().key(), c.key());
            let conc: Vec<_> = s.concurrent().iter().map(|r| r.key()).collect();
            assert_eq!(conc, vec![b.key()], "order {p:?}");
        }
    }

    #[test]
    fn long_chain_has_no_conflicts_and_pruned_ancestors_are_conservative() {
        // 12 sequential edits by alternating devices, history limit 5.
        let mut ops = Vec::new();
        for i in 0..12u64 {
            ops.push(reg(
                Some("v"),
                h(10 + i, 0),
                (i % 2) as u8 + 1,
                if i == 0 { None } else { Some(h(9 + i, 0)) },
            ));
        }
        let s = fold(&ops, 5);
        assert_eq!(s.history.len(), 5);
        assert!(
            s.concurrent().is_empty(),
            "retained versions form an unbroken chain to the winner"
        );
        // An old concurrent version that is retained but whose chain link is pruned stays visible.
        let stray = reg(Some("stray"), h(8, 0), 3, None);
        let mut s2 = s.clone();
        s2.apply(stray.clone(), 6);
        let keys: Vec<_> = s2.concurrent().iter().map(|r| r.key()).collect();
        assert_eq!(keys, vec![stray.key()]);
    }

    #[test]
    fn ambiguous_base_hlc_is_not_treated_as_an_ancestor() {
        // Two versions share hlc (5,0) from different devices; W is based on "(5,0)".
        let x = reg(Some("x"), h(5, 0), 1, None);
        let y = reg(Some("y"), h(5, 0), 2, None);
        let w = reg(Some("w"), h(9, 0), 3, Some(h(5, 0)));
        let s = fold(&[x.clone(), y.clone(), w], 5);
        assert_eq!(
            s.concurrent().len(),
            2,
            "cannot tell which one W saw, so show both"
        );
    }

    #[test]
    fn invalid_base_links_cannot_loop_or_panic() {
        let a = reg(Some("a"), h(5, 0), 1, Some(h(5, 0))); // base == own hlc
        let b = reg(Some("b"), h(6, 0), 2, Some(h(7, 0))); // base from the future
        let s = fold(&[a, b], 5);
        let _ = s.concurrent();
    }

    #[test]
    fn same_key_different_content_still_converges() {
        // A forked device id produced two different versions with the same (hlc, device).
        let p = reg(Some("p"), h(5, 0), 1, None);
        let q = reg(Some("q"), h(5, 0), 1, None);
        let top = reg(Some("t"), h(9, 0), 2, None);
        let s1 = fold(&[p.clone(), q.clone(), top.clone()], 5);
        let s2 = fold(&[top, q, p], 5);
        assert_eq!(s1, s2);
        assert_eq!(s1.history.len(), 1, "one entry per (hlc, device)");
    }
}
