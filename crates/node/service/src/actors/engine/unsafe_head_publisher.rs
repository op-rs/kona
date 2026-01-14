//! Unsafe head publisher trait and implementations.

use derive_more::Constructor;
use kona_protocol::L2BlockInfo;
use std::fmt::Debug;
use tokio::sync::watch;

/// Trait for publishing unsafe head updates.
///
/// This abstracts the mechanism for publishing unsafe head updates from the [`EngineProcessor`],
/// allowing for different publishing strategies to be injected.
///
/// [`EngineProcessor`]: crate::EngineProcessor
#[cfg_attr(test, mockall::automock)]
pub trait UnsafeHeadPublisher: Debug + Send + Sync {
    /// Publishes a new unsafe head if it has changed from the previous value.
    ///
    /// Returns `true` if the value was modified.
    fn publish_if_modified(&self, new_head: L2BlockInfo) -> bool;
}

/// A [`watch::Sender`]-based implementation of [`UnsafeHeadPublisher`].
///
/// This implementation uses a [`watch::Sender`] to broadcast unsafe head updates
/// to subscribers. It only sends updates when the new head differs from the current value.
#[derive(Debug, Constructor)]
pub struct WatchUnsafeHeadPublisher {
    /// The watch sender used to broadcast unsafe head updates.
    sender: watch::Sender<L2BlockInfo>,
}

impl UnsafeHeadPublisher for WatchUnsafeHeadPublisher {
    fn publish_if_modified(&self, new_head: L2BlockInfo) -> bool {
        self.sender.send_if_modified(|val| (*val != new_head).then(|| *val = new_head).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kona_protocol::BlockInfo;

    #[test]
    fn test_publish_if_modified_returns_true_when_changed() {
        let (tx, rx) = watch::channel(L2BlockInfo::default());
        let publisher = WatchUnsafeHeadPublisher::new(tx);

        let new_head = L2BlockInfo {
            block_info: BlockInfo { number: 1, ..Default::default() },
            ..Default::default()
        };
        assert!(publisher.publish_if_modified(new_head));
        assert_eq!(*rx.borrow(), new_head);
    }

    #[test]
    fn test_publish_if_modified_returns_false_when_unchanged() {
        let initial = L2BlockInfo {
            block_info: BlockInfo { number: 1, ..Default::default() },
            ..Default::default()
        };
        let (tx, _rx) = watch::channel(initial);
        let publisher = WatchUnsafeHeadPublisher::new(tx);

        assert!(!publisher.publish_if_modified(initial));
    }
}
