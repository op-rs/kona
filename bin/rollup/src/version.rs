//! Version information for kona-rollup.

#![allow(dead_code)]

/// The latest version from Cargo.toml.
pub(crate) const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The 8 character short SHA of the latest commit.
pub(crate) const VERGEN_GIT_SHA: &str = env!("VERGEN_GIT_SHA_SHORT");

/// The build timestamp.
pub(crate) const VERGEN_BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");

/// The target triple.
pub(crate) const VERGEN_CARGO_TARGET_TRIPLE: &str = env!("VERGEN_CARGO_TARGET_TRIPLE");

/// The build features.
pub(crate) const VERGEN_CARGO_FEATURES: &str = env!("VERGEN_CARGO_FEATURES");

/// The short version information for kona-rollup.
pub(crate) const SHORT_VERSION: &str = env!("KONA_ROLLUP_SHORT_VERSION");

/// The long version information for kona-rollup.
pub(crate) const LONG_VERSION: &str = concat!(
    env!("KONA_ROLLUP_LONG_VERSION_0"),
    "\n",
    env!("KONA_ROLLUP_LONG_VERSION_1"),
    "\n",
    env!("KONA_ROLLUP_LONG_VERSION_2"),
    "\n",
    env!("KONA_ROLLUP_LONG_VERSION_3"),
    "\n",
    env!("KONA_ROLLUP_LONG_VERSION_4")
);

/// The build profile name.
pub(crate) const BUILD_PROFILE_NAME: &str = env!("KONA_ROLLUP_BUILD_PROFILE");
