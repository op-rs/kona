#[cfg(test)]
use super::*;
use crate::actors::{
    MockBlockBuildingClient, MockOriginSelector, sequencer::test_util::test_actor,
};

#[tokio::test]
async fn test_build_unsealed_payload_prepare_payload_attributes_error() {
    let mut client = MockBlockBuildingClient::new();

    let unsafe_head = L2BlockInfo::default();
    client.expect_get_unsafe_head().times(1).return_once(move || Ok(unsafe_head));
    // Must not be called on critical error
    client.expect_start_build_block().times(0);

    let l1_origin = BlockInfo::default();
    let mut origin_selector = MockOriginSelector::new();
    origin_selector.expect_next_l1_origin().times(1).return_once(move |_, _| Ok(l1_origin));

    let mut actor = test_actor();
    actor.origin_selector = origin_selector;
    actor.block_building_client = client;

    let result = actor.build_unsealed_payload().await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, SequencerActorError::AttributesBuilder(PipelineErrorKind::Critical(_))));
}
