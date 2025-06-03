//! Chain Processor Module
//! This module implements the Chain Processor, which manages the nodes and process events per
//! chain. It provides a structured way to handle tasks, manage chains, and process blocks
//! in a supervisor environment.
mod chain;
pub use chain::ChainProcessor;

mod error;
pub use error::ChainProcessorError;

mod task;
pub use task::ChainProcessorTask;
