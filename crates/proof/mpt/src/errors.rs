//! Errors for the `kona-derive` crate.

use alloc::string::String;
use thiserror::Error;

/// A [Result] type alias where the error is [TrieNodeError].
pub type TrieNodeResult<T> = Result<T, TrieNodeError>;

/// An error type for [TrieNode] operations.
///
/// [TrieNode]: crate::TrieNode
#[derive(Error, Debug, PartialEq, Eq)]
pub enum TrieNodeError {
    /// Invalid trie node type encountered.
    #[error("Invalid trie node type encountered")]
    InvalidNodeType,
    /// Failed to decode trie node.
    #[error("Failed to decode trie node: {0}")]
    RLPError(alloy_rlp::Error),
    /// Key does not exist in trie.
    #[error("Key does not exist in trie.")]
    KeyNotFound,
    /// Trie node is not a leaf node.
    #[error("Trie provider error: {0}")]
    Provider(String),
}

/// A [Result] type alias where the error is [OrderedListWalkerError].
pub type OrderedListWalkerResult<T> = Result<T, OrderedListWalkerError>;

/// An error type for [OrderedListWalker] operations.
///
/// [OrderedListWalker]: crate::OrderedListWalker
#[derive(Error, Debug, PartialEq, Eq)]
pub enum OrderedListWalkerError {
    /// Iterator has already been hydrated, and cannot be re-hydrated until it is exhausted.
    #[error("Iterator has already been hydrated, and cannot be re-hydrated until it is exhausted")]
    AlreadyHydrated,
    /// Trie node error.
    #[error("{0}")]
    TrieNode(#[from] TrieNodeError),
}
