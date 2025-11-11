//! Contains all hardforks represented in the [crate::Hardfork] type.

use alloc::{sync::Arc, vec, vec::Vec};
use alloy_primitives::Bytes;

use crate::{Ecotone, Fjord, Hardfork as _, Interop, Isthmus, Jovian};
use kona_genesis::RollupConfig;

/// Optimism Hardforks
///
/// This type is used to encapsulate hardfork transactions.
/// It exposes methods that return hardfork upgrade transactions
/// as [`alloy_primitives::Bytes`].
///
/// # Example
///
/// Build ecotone hardfork upgrade transaction:
/// ```rust
/// use kona_hardforks::{Hardfork, Hardforks};
/// let ecotone_upgrade_tx = Hardforks::ECOTONE.txs();
/// assert_eq!(ecotone_upgrade_tx.collect::<Vec<_>>().len(), 6);
/// ```
///
/// Build fjord hardfork upgrade transactions:
/// ```rust
/// use kona_hardforks::{Hardfork, Hardforks};
/// let fjord_upgrade_txs = Hardforks::FJORD.txs();
/// assert_eq!(fjord_upgrade_txs.collect::<Vec<_>>().len(), 3);
/// ```
///
/// Build isthmus hardfork upgrade transaction:
/// ```rust
/// use kona_hardforks::{Hardfork, Hardforks};
/// let isthmus_upgrade_tx = Hardforks::ISTHMUS.txs();
/// assert_eq!(isthmus_upgrade_tx.collect::<Vec<_>>().len(), 8);
/// ```
///
/// Build interop hardfork upgrade transaction:
/// ```rust
/// use kona_hardforks::{Hardfork, Hardforks};
/// let interop_upgrade_tx = Hardforks::INTEROP.txs();
/// assert_eq!(interop_upgrade_tx.collect::<Vec<_>>().len(), 4);
/// ```
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct Hardforks;

impl Hardforks {
    /// The Ecotone hardfork upgrade transactions.
    pub const ECOTONE: Ecotone = Ecotone;

    /// The Fjord hardfork upgrade transactions.
    pub const FJORD: Fjord = Fjord;

    /// The Isthmus hardfork upgrade transactions.
    pub const ISTHMUS: Isthmus = Isthmus;

    /// The Jovian hardfork upgrade transactions.
    pub const JOVIAN: Jovian = Jovian;

    /// The Interop hardfork upgrade transactions.
    pub const INTEROP: Interop = Interop;

    /// Builds a vector with tokens for each active hardfork on the next l2 block.
    pub fn active_hardforks(rollup_cfg: Arc<RollupConfig>, next_l2_time: u64) -> Vec<Fork> {
        let mut hardforks = vec![];
        if rollup_cfg.is_ecotone_active(next_l2_time) {
            hardforks.push(Fork::Ecotone);
        }
        if rollup_cfg.is_fjord_active(next_l2_time) {
            hardforks.push(Fork::Fjord);
        }
        if rollup_cfg.is_isthmus_active(next_l2_time) {
            hardforks.push(Fork::Isthmus);
        }
        if rollup_cfg.is_jovian_active(next_l2_time) {
            hardforks.push(Fork::Jovian);
        }
        if rollup_cfg.is_interop_active(next_l2_time) {
            hardforks.push(Fork::Interop);
        }
        // continue ..
        hardforks
    }
}

/// A simple enum since trait `Hardfork`` is not trait safe.  This allows statically checking that
/// each fork has been handled.
#[derive(Debug)]
pub enum Fork {
    Ecotone,
    Fjord,
    Isthmus,
    Jovian,
    Interop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hardfork;
    use alloc::vec::Vec;

    #[test]
    fn test_hardforks() {
        let ecotone_upgrade_tx = Hardforks::ECOTONE.txs();
        assert_eq!(ecotone_upgrade_tx.collect::<Vec<_>>().len(), 6);

        let fjord_upgrade_txs = Hardforks::FJORD.txs();
        assert_eq!(fjord_upgrade_txs.collect::<Vec<_>>().len(), 3);

        let isthmus_upgrade_tx = Hardforks::ISTHMUS.txs();
        assert_eq!(isthmus_upgrade_tx.collect::<Vec<_>>().len(), 8);

        let jovian_upgrade_tx = Hardforks::JOVIAN.txs();
        assert_eq!(jovian_upgrade_tx.collect::<Vec<_>>().len(), 5);

        let interop_upgrade_tx = Hardforks::INTEROP.txs();
        assert_eq!(interop_upgrade_tx.collect::<Vec<_>>().len(), 4);
    }
}
