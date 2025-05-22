/// Key representing a particular head reference type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HeadRefKey {
    /// Latest unverified or unsafe head.
    Unsafe,

    /// Head block considered safe via local verification.
    LocalSafe,

    /// Head block confirmed safe via cross-chain sync.
    CrossSafe,

    /// Finalized head block.
    FinalizedL1,
}
