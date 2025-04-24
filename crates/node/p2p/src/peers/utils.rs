//! Utilities to translate types.

use discv5::{Enr, multiaddr::Protocol};
use libp2p::Multiaddr;

use super::PeerId;

/// Converts an [`Enr`] into a [`Multiaddr`].
pub fn enr_to_multiaddr(enr: &Enr) -> Option<Multiaddr> {
    if let Some(socket) = enr.tcp4_socket() {
        let mut addr = Multiaddr::from(*socket.ip());
        addr.push(Protocol::Tcp(socket.port()));
        return Some(addr);
    }
    if let Some(socket) = enr.tcp6_socket() {
        let mut addr = Multiaddr::from(*socket.ip());
        addr.push(Protocol::Tcp(socket.port()));
        return Some(addr);
    }
    None
}

/// Converts an uncompressed [`PeerId`] to a [`secp256k1::PublicKey`] by prepending the [`PeerId`]
/// bytes with the `SECP256K1_TAG_PUBKEY_UNCOMPRESSED` tag.
pub fn peer_id_to_secp256k1_pubkey(id: PeerId) -> Result<secp256k1::PublicKey, secp256k1::Error> {
    /// Tags the public key as uncompressed.
    ///
    /// See: <https://github.com/bitcoin-core/secp256k1/blob/master/include/secp256k1.h#L211>
    const SECP256K1_TAG_PUBKEY_UNCOMPRESSED: u8 = 4;

    let mut full_pubkey = [0u8; secp256k1::constants::UNCOMPRESSED_PUBLIC_KEY_SIZE];
    full_pubkey[0] = SECP256K1_TAG_PUBKEY_UNCOMPRESSED;
    full_pubkey[1..].copy_from_slice(id.as_slice());
    secp256k1::PublicKey::from_slice(&full_pubkey)
}

/// An error that can occur when converting a [`PeerId`] to a [`libp2p::PeerId`].
#[derive(Debug, thiserror::Error)]
pub enum PeerIdConversionError {
    /// The peer id is not valid and cannot be converted to a secp256k1 public key.
    #[error("Invalid peer id: {0}")]
    InvalidPeerId(secp256k1::Error),
    /// The secp256k1 public key cannot be converted to a libp2p peer id. This is a bug.
    #[error("Invalid conversion from secp256k1 public key to libp2p peer id: {0}. This is a bug.")]
    InvalidPublicKey(#[from] discv5::libp2p_identity::DecodingError),
}

/// Converts an uncoded [`PeerId`] to a [`libp2p::PeerId`]. These two types represent the same
/// underlying concept (secp256k1 public key) but using different encodings (the local [`PeerId`] is
/// the uncompressed representation of the public key, while the "p2plib" [`libp2p::PeerId`] is a
/// more complex representation, involving protobuf encoding and bitcoin encoding,  defined here: <https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md>).
pub fn local_id_to_p2p_id(peer_id: PeerId) -> Result<libp2p::PeerId, PeerIdConversionError> {
    // The libp2p library works with compressed public keys.
    let encoded_pk_bytes = peer_id_to_secp256k1_pubkey(peer_id)
        .map_err(PeerIdConversionError::InvalidPeerId)?
        .serialize();
    let pk: discv5::libp2p_identity::PublicKey =
        discv5::libp2p_identity::secp256k1::PublicKey::try_from_bytes(&encoded_pk_bytes)?.into();

    Ok(pk.to_peer_id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PeerId;
    use alloy_primitives::hex::FromHex;

    #[test]
    fn test_convert_local_peer_id_to_multi_peer_id() {
        let p2p_keypair = discv5::libp2p_identity::secp256k1::Keypair::generate();
        let uncompressed = p2p_keypair.public().to_bytes_uncompressed();
        let local_peer_id = PeerId::from_slice(&uncompressed[1..]);

        // We need to convert the local peer id (uncompressed secp256k1 public key) to a libp2p
        // peer id (protocol buffer encoded public key).
        let peer_id = local_id_to_p2p_id(local_peer_id).unwrap();

        let p2p_public_key: discv5::libp2p_identity::PublicKey =
            p2p_keypair.public().clone().into();

        assert_eq!(peer_id, p2p_public_key.to_peer_id());
    }

    #[test]
    fn test_hardcoded_peer_id() {
        const PUB_KEY_STR: &str = "548f715f3fc388a7c917ba644a2f16270f1ede48a5d88a4d14ea287cc916068363f3092e39936f1a3e7885198bef0e5af951f1d7b1041ce8ba4010917777e71f";
        let pub_key = PeerId::from_hex(PUB_KEY_STR).unwrap();

        // We need to convert the local peer id (uncompressed secp256k1 public key) to a libp2p
        // peer id (protocol buffer encoded public key).
        let peer_id = local_id_to_p2p_id(pub_key).unwrap();

        let uncompressed_pub_key = peer_id_to_secp256k1_pubkey(pub_key).unwrap();

        let p2p_public_key: discv5::libp2p_identity::PublicKey =
            discv5::libp2p_identity::secp256k1::PublicKey::try_from_bytes(
                &uncompressed_pub_key.serialize(),
            )
            .unwrap()
            .into();

        assert_eq!(peer_id, p2p_public_key.to_peer_id());
    }
}
