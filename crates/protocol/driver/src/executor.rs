//! An abstraction for the driver's block executor.

use alloc::boxed::Box;
use core::{
    error::Error,
    fmt::{Debug, Display},
};
use kona_executor::ExecutionArtifacts;

use alloc::string::ToString;
use alloy_consensus::{Header, Sealed};
use alloy_primitives::B256;
use async_trait::async_trait;
use op_alloy_rpc_types_engine::OpPayloadAttributes;

/// Executor
///
/// Abstracts block execution by the driver.
#[async_trait]
pub trait Executor {
    /// The error type for the Executor.
    type Error: Error + Debug + Display + ToString;

    /// Waits for the executor to be ready.
    async fn wait_until_ready(&mut self);

    /// Updates the safe header.
    fn update_safe_head(&mut self, header: Sealed<Header>);

    /// Execute the gicen [OpPayloadAttributes].
    async fn execute_payload(
        &mut self,
        attributes: OpPayloadAttributes,
    ) -> Result<ExecutionArtifacts, Self::Error>;

    /// Computes the output root.
    /// Expected to be called after the payload has been executed.
    fn compute_output_root(&mut self) -> Result<B256, Self::Error>;
}
