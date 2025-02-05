#![doc = include_str!("../README.md")]
#![warn(missing_debug_implementations, missing_docs, unreachable_pub, rustdoc::all)]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(not(any(test, feature = "arbitrary")), no_std)]

extern crate alloc;

mod pre_state;
pub use pre_state::{
    OptimisticBlock, PreState, TransitionState, INVALID_TRANSITION_HASH, TRANSITION_STATE_MAX_STEPS,
};

mod hint;
pub use hint::HintType;

mod provider;
pub use provider::OracleInteropProvider;

pub mod boot;
pub use boot::BootInfo;

mod consolidation;
pub use consolidation::{ConsolidationError, SuperchainConsolidator};
