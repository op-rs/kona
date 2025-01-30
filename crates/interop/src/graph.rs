//! Interop [MessageGraph].

use crate::{
    errors::{MessageGraphError, MessageGraphResult},
    message::{extract_executing_messages, EnrichedExecutingMessage},
    traits::InteropProvider,
    RawMessagePayload,
};
use alloc::vec::Vec;
use alloy_consensus::{Header, Sealed};
use alloy_primitives::{hex, keccak256};
use tracing::{info, warn};

/// The message graph represents a set of blocks at a given timestamp and the interop
/// dependencies between them.
///
/// This structure is used to determine whether or not any interop messages are invalid within the
/// set of blocks within the graph. An "invalid message" is one that was relayed from one chain to
/// another, but the original [MessageIdentifier] is not present within the graph or from a
/// dependency referenced via the [InteropProvider] (or otherwise is invalid, such as being older
/// than the message expiry window).
///
/// Message validity rules: <https://specs.optimism.io/interop/messaging.html#invalid-messages>
///
/// [MessageIdentifier]: crate::MessageIdentifier
#[derive(Debug)]
pub struct MessageGraph<'a, P> {
    /// The horizon timestamp is the highest timestamp of all blocks containing [ExecutingMessage]s
    /// within the graph.
    ///
    /// [ExecutingMessage]: crate::ExecutingMessage
    horizon_timestamp: u64,
    /// The edges within the graph.
    ///
    /// These are derived from the transactions within the blocks.
    messages: Vec<EnrichedExecutingMessage>,
    /// The data provider for the graph. Required for fetching headers, receipts and remote
    /// messages within history during resolution.
    provider: &'a P,
}

impl<'a, P> MessageGraph<'a, P>
where
    P: InteropProvider,
{
    /// Derives the edges from the blocks within the graph by scanning all receipts within the
    /// blocks and searching for [ExecutingMessage]s.
    ///
    /// [ExecutingMessage]: crate::ExecutingMessage
    pub async fn derive(
        blocks: &[(u64, Sealed<Header>)],
        provider: &'a P,
    ) -> MessageGraphResult<Self, P> {
        info!(
            target: "message-graph",
            "Deriving message graph from {} blocks.",
            blocks.len()
        );

        // Get the highest timestamp from the blocks. This serves as the horizon timestamp for the
        // graph.
        let horizon_timestamp = blocks
            .iter()
            .map(|(_, header)| header.inner().timestamp)
            .max()
            .ok_or(MessageGraphError::EmptyDependencySet)?;

        let mut messages = Vec::with_capacity(blocks.len());
        for (chain_id, header) in blocks.iter() {
            let receipts = provider.receipts_by_hash(*chain_id, header.hash()).await?;
            let executing_messages = extract_executing_messages(receipts.as_slice());

            messages.extend(
                executing_messages
                    .into_iter()
                    .map(|message| EnrichedExecutingMessage::new(message, *chain_id)),
            );
        }

        info!(
            target: "message-graph",
            "Derived {} executing messages from {} blocks.",
            messages.len(),
            blocks.len()
        );
        Ok(Self { horizon_timestamp, messages, provider })
    }

    /// Checks the validity of all messages within the graph.
    pub async fn resolve(mut self) -> MessageGraphResult<(), P> {
        info!(
            target: "message-graph",
            "Checking the message graph for invalid messages."
        );

        // Reduce the graph to remove all valid messages.
        self.reduce().await?;

        // Check if the graph is now empty. If not, there are invalid messages.
        if !self.messages.is_empty() {
            // Collect the chain IDs for all blocks containing invalid messages.
            let mut bad_block_chain_ids =
                self.messages.into_iter().map(|e| e.executing_chain_id).collect::<Vec<_>>();
            bad_block_chain_ids.dedup_by(|a, b| a == b);

            warn!(
                target: "message-graph",
                "Failed to reduce the message graph entirely. Invalid messages found in chains {}",
                bad_block_chain_ids
                    .iter()
                    .map(|id| alloc::format!("{}", id))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            // Return an error with the chain IDs of the blocks containing invalid messages.
            return Err(MessageGraphError::InvalidMessages(bad_block_chain_ids));
        }

        Ok(())
    }

    /// Attempts to remove as many edges from the graph as possible by resolving the dependencies
    /// of each message. If a message cannot be resolved, it is considered invalid. After this
    /// function is called, any outstanding messages are invalid.
    async fn reduce(&mut self) -> MessageGraphResult<(), P> {
        // Create a new vector to store invalid edges
        let mut invalid_messages = Vec::with_capacity(self.messages.len());

        // Prune all valid edges.
        for message in core::mem::take(&mut self.messages) {
            if let Err(e) = self.check_single_dependency(&message).await {
                warn!(
                    target: "message-graph",
                    "Invalid ExecutingMessage found - relayed on chain {} with message hash {}.",
                    message.executing_chain_id,
                    hex::encode(message.inner.msgHash)
                );
                warn!("Invalid message error: {}", e);
                invalid_messages.push(message);
            }
        }

        info!(
            target: "message-graph",
            "Successfully reduced the message graph. {} invalid messages found.",
            invalid_messages.len()
        );

        // Replace the old edges with the filtered list
        self.messages = invalid_messages;

        Ok(())
    }

    /// Checks the dependency of a single [EnrichedExecutingMessage]. If the message's dependencies
    /// are unavailable, the message is considered invalid and an [Err] is returned.
    async fn check_single_dependency(
        &self,
        message: &EnrichedExecutingMessage,
    ) -> MessageGraphResult<(), P> {
        // ChainID Invariant: The chain id of the initiating message MUST be in the dependency set
        // This is enforced implicitly by the graph constructor and the provider.

        // Timestamp invariant: The timestamp at the time of inclusion of the initiating message
        // MUST be less than or equal to the timestamp of the executing message as well as greater
        // than or equal to the Interop Start Timestamp.
        if message.inner.id.timestamp.saturating_to::<u64>() > self.horizon_timestamp {
            // TODO(interop): Also need to check for the interop start timestamp. Requires
            // `RollupConfig`s for each chain.
            return Err(MessageGraphError::MessageInFuture(
                self.horizon_timestamp,
                message.inner.id.timestamp.saturating_to(),
            ));
        }

        // Fetch the header & receipts for the message's claimed origin block on the remote chain.
        let remote_header = self
            .provider
            .header_by_number(
                message.inner.id.chainId.saturating_to(),
                message.inner.id.blockNumber.saturating_to(),
            )
            .await?;
        let remote_receipts = self
            .provider
            .receipts_by_number(
                message.inner.id.chainId.saturating_to(),
                message.inner.id.blockNumber.saturating_to(),
            )
            .await?;

        // Find the log that matches the message's claimed log index. Note that the
        // log index is global to the block, so we chain the full block's logs together
        // to find it.
        let remote_log = remote_receipts
            .iter()
            .flat_map(|receipt| receipt.logs())
            .nth(message.inner.id.logIndex.saturating_to())
            .ok_or(MessageGraphError::RemoteMessageNotFound(
                message.inner.id.chainId.to(),
                message.inner.msgHash,
            ))?;

        // Validate the message's origin is correct.
        if remote_log.address != message.inner.id.origin {
            return Err(MessageGraphError::InvalidMessageOrigin(
                message.inner.id.origin,
                remote_log.address,
            ));
        }

        // Validate that the message hash is correct.
        let remote_message = RawMessagePayload::from(remote_log);
        let remote_message_hash = keccak256(remote_message.as_ref());
        if remote_message_hash != message.inner.msgHash {
            return Err(MessageGraphError::InvalidMessageHash(
                message.inner.msgHash,
                remote_message_hash,
            ));
        }

        // Validate that the timestamp of the block header containing the log is correct.
        if remote_header.timestamp != message.inner.id.timestamp.saturating_to::<u64>() {
            return Err(MessageGraphError::InvalidMessageTimestamp(
                message.inner.id.timestamp.saturating_to::<u64>(),
                remote_header.timestamp,
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::MessageGraph;
    use crate::{test_util::SuperchainBuilder, MessageGraphError};
    use alloy_primitives::{hex, keccak256, Address};

    const MESSAGE: [u8; 4] = hex!("deadbeef");

    #[tokio::test]
    async fn test_derive_and_reduce_simple_graph() {
        let mut superchain = SuperchainBuilder::new(0);

        superchain.chain(1).add_initiating_message(MESSAGE.into());
        superchain.chain(2).add_executing_message(keccak256(MESSAGE), 0, 1, 0);

        let (headers, provider) = superchain.build();

        let graph = MessageGraph::derive(headers.as_slice(), &provider).await.unwrap();
        graph.resolve().await.unwrap();
    }

    #[tokio::test]
    async fn test_derive_and_reduce_cyclical_graph() {
        let mut superchain = SuperchainBuilder::new(0);

        superchain.chain(1).add_initiating_message(MESSAGE.into()).add_executing_message(
            keccak256(MESSAGE),
            1,
            2,
            0,
        );
        superchain
            .chain(2)
            .add_executing_message(keccak256(MESSAGE), 0, 1, 0)
            .add_initiating_message(MESSAGE.into());

        let (headers, provider) = superchain.build();

        let graph = MessageGraph::derive(headers.as_slice(), &provider).await.unwrap();
        graph.resolve().await.unwrap();
    }

    #[tokio::test]
    async fn test_derive_and_reduce_simple_graph_remote_message_not_found() {
        let mut superchain = SuperchainBuilder::new(0);

        superchain.chain(1);
        superchain.chain(2).add_executing_message(keccak256(MESSAGE), 0, 1, 0);

        let (headers, provider) = superchain.build();

        let graph = MessageGraph::derive(headers.as_slice(), &provider).await.unwrap();
        assert_eq!(graph.resolve().await.unwrap_err(), MessageGraphError::InvalidMessages(vec![2]));
    }

    #[tokio::test]
    async fn test_derive_and_reduce_simple_graph_invalid_chain_id() {
        let mut superchain = SuperchainBuilder::new(0);

        superchain.chain(1).add_initiating_message(MESSAGE.into());
        superchain.chain(2).add_executing_message(keccak256(MESSAGE), 0, 2, 0);

        let (headers, provider) = superchain.build();

        let graph = MessageGraph::derive(headers.as_slice(), &provider).await.unwrap();
        assert_eq!(graph.resolve().await.unwrap_err(), MessageGraphError::InvalidMessages(vec![2]));
    }

    #[tokio::test]
    async fn test_derive_and_reduce_simple_graph_invalid_log_index() {
        let mut superchain = SuperchainBuilder::new(0);

        superchain.chain(1).add_initiating_message(MESSAGE.into());
        superchain.chain(2).add_executing_message(keccak256(MESSAGE), 1, 1, 0);

        let (headers, provider) = superchain.build();

        let graph = MessageGraph::derive(headers.as_slice(), &provider).await.unwrap();
        assert_eq!(graph.resolve().await.unwrap_err(), MessageGraphError::InvalidMessages(vec![2]));
    }

    #[tokio::test]
    async fn test_derive_and_reduce_simple_graph_invalid_message_hash() {
        let mut superchain = SuperchainBuilder::new(0);

        superchain.chain(1).add_initiating_message(MESSAGE.into());
        superchain.chain(2).add_executing_message(keccak256(hex!("0badc0de")), 0, 1, 0);

        let (headers, provider) = superchain.build();

        let graph = MessageGraph::derive(headers.as_slice(), &provider).await.unwrap();
        assert_eq!(graph.resolve().await.unwrap_err(), MessageGraphError::InvalidMessages(vec![2]));
    }

    #[tokio::test]
    async fn test_derive_and_reduce_simple_graph_invalid_origin_address() {
        let mut superchain = SuperchainBuilder::new(0);

        superchain.chain(1).add_initiating_message(MESSAGE.into());
        superchain.chain(2).add_executing_message_with_origin(
            keccak256(MESSAGE),
            Address::left_padding_from(&[0x01]),
            0,
            1,
            0,
        );

        let (headers, provider) = superchain.build();

        let graph = MessageGraph::derive(headers.as_slice(), &provider).await.unwrap();
        assert_eq!(graph.resolve().await.unwrap_err(), MessageGraphError::InvalidMessages(vec![2]));
    }
}
