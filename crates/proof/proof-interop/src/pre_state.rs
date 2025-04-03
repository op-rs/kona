//! Types for the pre-state claims used in the interop proof.

use alloc::vec::Vec;
use alloy_primitives::{B256, Bytes, b256, keccak256};
use alloy_rlp::{Buf, Decodable, Encodable, Header, RlpDecodable, RlpEncodable};
use kona_interop::{OutputRootWithChain, SUPER_ROOT_VERSION, SuperRoot};
use serde::{Deserialize, Serialize};

/// The current [TransitionState] encoding format version.
pub(crate) const TRANSITION_STATE_VERSION: u8 = 255;

/// The maximum number of steps allowed in a [TransitionState].
pub const TRANSITION_STATE_MAX_STEPS: u64 = 2u64.pow(7) - 1;

/// The [Bytes] representation of the string "invalid".
pub const INVALID_TRANSITION: Bytes = Bytes::from_static(b"invalid");

/// `keccak256("invalid")`
pub const INVALID_TRANSITION_HASH: B256 =
    b256!("ffd7db0f9d5cdeb49c4c9eba649d4dc6d852d64671e65488e57f58584992ac68");

/// The [PreState] of the interop proof program can be one of two types: a [SuperRoot] or a
/// [TransitionState]. The [SuperRoot] is the canonical state of the superchain, while the
/// [TransitionState] is a super-structure of the [SuperRoot] that represents the progress of a
/// pending superchain state transition from one [SuperRoot] to the next.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum PreState {
    /// The canonical state of the superchain.
    SuperRoot(SuperRoot),
    /// The progress of a pending superchain state transition.
    TransitionState(TransitionState),
}

impl PreState {
    /// Hashes the encoded [PreState] using [keccak256].
    pub fn hash(&self) -> B256 {
        let mut rlp_buf = Vec::with_capacity(self.length());
        self.encode(&mut rlp_buf);
        keccak256(&rlp_buf)
    }

    /// Returns the timestamp of the [PreState].
    pub const fn timestamp(&self) -> u64 {
        match self {
            Self::SuperRoot(super_root) => super_root.timestamp,
            Self::TransitionState(transition_state) => transition_state.pre_state.timestamp,
        }
    }

    /// Returns the active L2 output root hash of the [PreState]. This is the output root that
    /// represents the pre-state of the chain that is to be committed to in the next transition
    /// step, or [None] if the [PreState] has already been fully saturated.
    pub fn active_l2_output_root(&self) -> Option<&OutputRootWithChain> {
        match self {
            Self::SuperRoot(super_root) => super_root.output_roots.first(),
            Self::TransitionState(transition_state) => {
                transition_state.pre_state.output_roots.get(transition_state.step as usize)
            }
        }
    }

    /// Returns the active L2 chain ID of the [PreState]. This is the chain ID of the output root
    /// that is to be committed to in the next transition step, or [None] if the [PreState]
    /// has already been fully saturated.
    pub fn active_l2_chain_id(&self) -> Option<u64> {
        self.active_l2_output_root().map(|output_root| output_root.chain_id)
    }

    /// Transitions to the next state, appending the [OptimisticBlock] to the pending progress.
    pub fn transition(self, optimistic_block: Option<OptimisticBlock>) -> Option<Self> {
        match self {
            Self::SuperRoot(super_root) => Some(Self::TransitionState(TransitionState::new(
                super_root,
                alloc::vec![optimistic_block?],
                1,
            ))),
            Self::TransitionState(mut transition_state) => {
                // If the transition state's pending progress contains the same number of states as
                // the pre-state's output roots already, then we can either no-op
                // the transition or finalize it.
                if transition_state.pending_progress.len() ==
                    transition_state.pre_state.output_roots.len()
                {
                    if transition_state.step == TRANSITION_STATE_MAX_STEPS {
                        let super_root = SuperRoot::new(
                            transition_state.pre_state.timestamp + 1,
                            transition_state
                                .pending_progress
                                .iter()
                                .zip(transition_state.pre_state.output_roots.iter())
                                .map(|(optimistic_block, pre_state_output)| {
                                    OutputRootWithChain::new(
                                        pre_state_output.chain_id,
                                        optimistic_block.output_root,
                                    )
                                })
                                .collect(),
                        );
                        return Some(Self::SuperRoot(super_root));
                    } else {
                        transition_state.step += 1;
                        return Some(Self::TransitionState(transition_state));
                    };
                }

                transition_state.pending_progress.push(optimistic_block?);
                transition_state.step += 1;
                Some(Self::TransitionState(transition_state))
            }
        }
    }
}

impl Encodable for PreState {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        match self {
            Self::SuperRoot(super_root) => {
                super_root.encode(out);
            }
            Self::TransitionState(transition_state) => {
                transition_state.encode(out);
            }
        }
    }
}

impl Decodable for PreState {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        if buf.is_empty() {
            return Err(alloy_rlp::Error::UnexpectedLength);
        }

        match buf[0] {
            TRANSITION_STATE_VERSION => {
                let transition_state = TransitionState::decode(buf)?;
                Ok(Self::TransitionState(transition_state))
            }
            SUPER_ROOT_VERSION => {
                let super_root =
                    SuperRoot::decode(buf).map_err(|_| alloy_rlp::Error::UnexpectedString)?;
                Ok(Self::SuperRoot(super_root))
            }
            _ => Err(alloy_rlp::Error::Custom("invalid version byte")),
        }
    }
}

/// The [TransitionState] is a super-structure of the [SuperRoot] that represents the progress of a
/// pending superchain state transition from one [SuperRoot] to the next.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct TransitionState {
    /// The canonical pre-state super root commitment.
    pub pre_state: SuperRoot,
    /// The progress that has been made in the pending superchain state transition.
    pub pending_progress: Vec<OptimisticBlock>,
    /// The step number of the pending superchain state transition.
    pub step: u64,
}

impl TransitionState {
    /// Create a new [TransitionState] with the given pre-state, pending progress, and step number.
    pub const fn new(
        pre_state: SuperRoot,
        pending_progress: Vec<OptimisticBlock>,
        step: u64,
    ) -> Self {
        Self { pre_state, pending_progress, step }
    }

    /// Hashes the encoded [TransitionState] using [keccak256].
    pub fn hash(&self) -> B256 {
        let mut rlp_buf = Vec::with_capacity(self.length());
        self.encode(&mut rlp_buf);
        keccak256(&rlp_buf)
    }

    /// Returns the RLP payload length of the [TransitionState].
    pub fn payload_length(&self) -> usize {
        Header { list: false, payload_length: self.pre_state.encoded_length() }.length() +
            self.pre_state.encoded_length() +
            self.pending_progress.length() +
            self.step.length()
    }
}

impl Encodable for TransitionState {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        out.put_u8(TRANSITION_STATE_VERSION);

        Header { list: true, payload_length: self.payload_length() }.encode(out);

        // The pre-state has special encoding, since it is not RLP. We encode the structure, and
        // then encode it as a RLP string.
        let mut pre_state_buf = Vec::new();
        self.pre_state.encode(&mut pre_state_buf);
        Bytes::from(pre_state_buf).encode(out);

        self.pending_progress.encode(out);
        self.step.encode(out);
    }
}

impl Decodable for TransitionState {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        if buf.is_empty() {
            return Err(alloy_rlp::Error::UnexpectedLength);
        }

        let version = buf[0];
        if version != TRANSITION_STATE_VERSION {
            return Err(alloy_rlp::Error::Custom("invalid version byte"));
        }
        buf.advance(1);

        // Decode the RLP header.
        let header = Header::decode(buf)?;
        if !header.list {
            return Err(alloy_rlp::Error::UnexpectedString);
        }

        // The pre-state has special decoding, since it is not RLP. We decode the RLP string, and
        // then decode the structure.
        let pre_state_buf = Bytes::decode(buf)?;
        let pre_state = SuperRoot::decode(&mut pre_state_buf.as_ref())
            .map_err(|_| alloy_rlp::Error::UnexpectedString)?;

        // The rest of the fields are RLP encoded as normal.
        let pending_progress = Vec::<OptimisticBlock>::decode(buf)?;
        let step = u64::decode(buf)?;

        Ok(Self { pre_state, pending_progress, step })
    }
}

/// A wrapper around a pending output root hash with the block hash it commits to.
#[derive(
    Default, Debug, Clone, Eq, PartialEq, RlpEncodable, RlpDecodable, Serialize, Deserialize,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct OptimisticBlock {
    /// The block hash of the output root.
    pub block_hash: B256,
    /// The output root hash.
    pub output_root: B256,
}

impl OptimisticBlock {
    /// Create a new [OptimisticBlock] with the given block hash and output root hash.
    pub const fn new(block_hash: B256, output_root: B256) -> Self {
        Self { block_hash, output_root }
    }
}

#[cfg(test)]
mod test {
    use super::{OptimisticBlock, SuperRoot, TransitionState};
    use alloc::{vec, vec::Vec};
    use alloy_primitives::B256;
    use alloy_rlp::{Decodable, Encodable};
    use kona_interop::OutputRootWithChain;

    #[test]
    fn test_static_transition_state_roundtrip() {
        let transition_state = TransitionState::new(
            SuperRoot::new(
                10,
                vec![
                    (OutputRootWithChain::new(1, B256::default())),
                    (OutputRootWithChain::new(2, B256::default())),
                ],
            ),
            vec![OptimisticBlock::default(), OptimisticBlock::default()],
            1,
        );

        let mut rlp_buf = Vec::with_capacity(transition_state.length());
        transition_state.encode(&mut rlp_buf);

        assert_eq!(transition_state, TransitionState::decode(&mut rlp_buf.as_slice()).unwrap());
    }

    #[test]
    #[cfg(feature = "arbitrary")]
    fn test_arbitrary_pre_state_roundtrip() {
        use arbitrary::Arbitrary;
        use rand::Rng;
        let mut bytes = [0u8; 1024];
        rand::rng().fill(bytes.as_mut_slice());
        let pre_state =
            super::PreState::arbitrary(&mut arbitrary::Unstructured::new(&bytes)).unwrap();

        let mut rlp_buf = Vec::with_capacity(pre_state.length());
        pre_state.encode(&mut rlp_buf);
        assert_eq!(pre_state, super::PreState::decode(&mut rlp_buf.as_slice()).unwrap());
    }

    #[test]
    #[cfg(feature = "arbitrary")]
    fn test_arbitrary_transition_state_roundtrip() {
        use arbitrary::Arbitrary;
        use rand::Rng;
        let mut bytes = [0u8; 1024];
        rand::rng().fill(bytes.as_mut_slice());
        let transition_state =
            TransitionState::arbitrary(&mut arbitrary::Unstructured::new(&bytes)).unwrap();

        let mut rlp_buf = Vec::with_capacity(transition_state.length());
        transition_state.encode(&mut rlp_buf);
        assert_eq!(transition_state, TransitionState::decode(&mut rlp_buf.as_slice()).unwrap());
    }
}
