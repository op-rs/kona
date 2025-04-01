//! Module containing consensus-layer gossipsub for optimism.

mod behaviour;
pub use behaviour::{Behaviour, BehaviourError};

mod config;
pub use config::{
    DEFAULT_MESH_D, DEFAULT_MESH_DHI, DEFAULT_MESH_DLAZY, DEFAULT_MESH_DLO,
    GLOBAL_VALIDATE_THROTTLE, GOSSIP_HEARTBEAT, MAX_GOSSIP_SIZE, MAX_OUTBOUND_QUEUE,
    MAX_VALIDATE_QUEUE, MIN_GOSSIP_SIZE, PEER_SCORE_INSPECT_FREQUENCY, SEEN_MESSAGES_TTL,
    default_config, default_config_builder,
};

mod builder;
pub use builder::GossipDriverBuilder;

mod error;
pub use error::{GossipDriverBuilderError, HandlerEncodeError, PublishError};

mod event;
pub use event::Event;

mod handler;
pub use handler::{BlockHandler, Handler};

mod driver;
pub use driver::GossipDriver;
