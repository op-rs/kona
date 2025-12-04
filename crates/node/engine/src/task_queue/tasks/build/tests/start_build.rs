//! Tests for BuildTask::start_build

use alloy_primitives::B256;
use alloy_rpc_types_engine::{ForkchoiceUpdated, PayloadId, PayloadStatus, PayloadStatusEnum};
use kona_genesis::RollupConfig;
use std::sync::Arc;

use crate::{
    BuildTask,
    test_utils::{
        TestAttributesBuilder, TestEngineStateBuilder, test_block_info, test_engine_client_builder,
    },
};

#[tokio::test]
async fn test_start_build_happy_path() {
    // Setup: Create a mock engine client that will return a successful forkchoice update
    let payload_id = PayloadId::new([1u8; 8]);
    let forkchoice_updated = ForkchoiceUpdated {
        payload_status: PayloadStatus {
            status: PayloadStatusEnum::Valid,
            latest_valid_hash: Some(B256::ZERO),
        },
        payload_id: Some(payload_id),
    };

    // Use v2 because the default RollupConfig will trigger v2 for these timestamps
    let client = test_engine_client_builder()
        .with_fork_choice_updated_v2_response(forkchoice_updated)
        .build();

    // Setup: Create an engine state with proper block hierarchy
    // unsafe_head (block 1) should be at or ahead of finalized_head (block 0)
    let parent_block = test_block_info(0);
    let unsafe_block = test_block_info(1);

    let state = TestEngineStateBuilder::new()
        .with_unsafe_head(unsafe_block)
        .with_safe_head(parent_block)
        .with_finalized_head(parent_block)
        .build();

    // Setup: Create attributes for building block 1
    let attributes = TestAttributesBuilder::new()
        .with_parent(parent_block)
        .with_timestamp(unsafe_block.block_info.timestamp)
        .build();

    // Setup: Create the build task
    let task = BuildTask::new(
        Arc::new(client.clone()),
        Arc::new(RollupConfig::default()),
        attributes.clone(),
        None,
    );

    // Execute: Call start_build
    let result = task.start_build(&state, &client, attributes).await;

    // Assert: The build should succeed and return the expected payload ID
    assert!(result.is_ok(), "start_build should succeed: {:?}", result.err());
    assert_eq!(result.unwrap(), payload_id, "Should return the correct payload ID");
}
