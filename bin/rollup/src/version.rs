//! Version information for kona-node.

/// The short version information for kona-node.
pub(crate) const SHORT_VERSION: &str = env!("KONA_NODE_SHORT_VERSION");

/// The long version information for kona-node.
pub(crate) const LONG_VERSION: &str = concat!(
    env!("KONA_NODE_LONG_VERSION_0"),
    "\n",
    env!("KONA_NODE_LONG_VERSION_1"),
    "\n",
    env!("KONA_NODE_LONG_VERSION_2"),
    "\n",
    env!("KONA_NODE_LONG_VERSION_3"),
    "\n",
    env!("KONA_NODE_LONG_VERSION_4")
);
