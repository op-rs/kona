use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_types::BlockReplacement;

/// Represents node events that [`ManagedNode`](`super::ManagedNode`) emits.
/// These events are used to notify the supervisor about changes in block states,
/// such as unsafe blocks, safe blocks, or block replacements.
/// Each event carries relevant information about the block involved,
/// allowing the supervisor to take appropriate actions based on the event type.
#[derive(Debug, Clone)]
pub enum NodeEvent {
    /// An unsafe block event, indicating that a new unsafe block has been detected.
    UnsafeBlock {
        /// The [`BlockInfo`] of the unsafe block.
        block: BlockInfo,
    },
    /// A derived block event, indicating that a new derived block has been detected.
    DerivedBlock {
        /// The [`DerivedRefPair`] containing the derived block and its source block.
        derived_ref_pair: DerivedRefPair,
    },

    /// A derivation origin update event, indicating that the origin for derived blocks has changed.
    DerivationOriginUpdate {
        /// The [`BlockInfo`] of the block that is the new derivation origin.
        origin: BlockInfo,
    },

    /// A block replacement event, indicating that a block has been replaced with a new one.
    BlockReplaced {
        /// The [`BlockReplacement`] containing the replacement block and the invalidated block
        /// hash.
        replacement: BlockReplacement,
    },
}
