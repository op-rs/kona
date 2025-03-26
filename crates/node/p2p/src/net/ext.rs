//! Contains an externally-facing api for the [`crate::Network`].
//!
//! The external api is used to service requests made by the RPC.

use kona_rpc::PeerInfo;
use tokio::sync::{mpsc::Receiver, oneshot::Sender};
use crate::{GossipDriver, Discv5Handler};

/// A network RPC Request.
#[derive(Debug)]
pub enum NetRpcRequest {
    /// Returns [`PeerInfo`] for the [`crate::Network`].
    PeerInfo(Sender<PeerInfo>),
}

/// An RPC request handler for the [`crate::Network`].
#[derive(Debug)]
pub struct NetworkRpcHandler {
    /// The inner [`Receiver`]
    pub receiver: Receiver<NetRpcRequest>,
}

impl Default for NetworkRpcHandler {
    fn default() -> Self {
        let (_, receiver) = tokio::sync::mpsc::channel(1);
        Self { receiver }
    }
}

impl NetworkRpcHandler {
    /// Constructs a new [`NetworkRpcHandler`] given a receiver channel.
    pub const fn new(receiver: Receiver<NetRpcRequest>) -> Self {
        Self { receiver }
    }
}

impl NetworkRpcHandler {
    /// Receives on the inner channel and handles the request.
    pub async fn recv(&mut self) -> Option<NetRpcRequest> {
        self.receiver.recv().await
    }

    /// Handles a [`NetRpcRequest`].
    pub fn handle(req: NetRpcRequest, disc: &Discv5Handler, gossip: &GossipDriver) {
        match req {
            NetRpcRequest::PeerInfo(sender) => {
                let peer_info = Self::peer_info(disc, gossip);
                if let Err(e) = sender.send(peer_info) {
                    warn!("Failed to send peer info through response channel: {:?}", e);
                }
            }
        }
    }

    /// Returns the [`PeerInfo`].
    pub fn peer_info(_disc: &Discv5Handler, _gossip: &GossipDriver) -> PeerInfo {
        tracing::info!("inside network peer info rpc handler");
        // use std::string::ToString;
        // let peer_id = gossip.local_peer_id().to_string();
        // let enr = disc.local_enr().await.unwrap();
        // let node_id = disc.local_enr().await.unwrap().id().unwrap_or_default();
        // let addresses = gossip.swarm.external_addresses().map(|a| a.to_string()).collect::<Vec<String>>();
        //
        // // Grab the local
        // PeerInfo {
        //     peer_id,
        //     node_id,
        //     user_agent: "kona".to_string(),
        //     protocol_version: "1".to_string(),
        //     enr: enr.to_string(),
        //     addresses,
        //     protocols: None, // TODO: peer supported protocols
        //     connectedness: kona_rpc::Connectedness::Connected,
        //     direction: kona_rpc::Direction::Inbound,
        //     protected: false,
        //     chain_id: 0, // TODO: chain id
        //     latency: 0,
        //     gossip_blocks: false,
        //     peer_scores: kona_rpc::PeerScores {
        //         gossip: kona_rpc::GossipScores {
        //             total: 0 as f64,
        //             blocks: kona_rpc::TopicScores {
        //                 time_in_mesh: 0 as f64,
        //                 first_message_deliveries: 0 as f64,
        //                 mesh_message_deliveries: 0 as f64,
        //                 invalid_message_deliveries: 0 as f64,
        //             },
        //             ip_colocation_factor: 0 as f64,
        //             behavioral_penalty: 0 as f64,
        //         },
        //         req_resp: kona_rpc::ReqRespScores {
        //             valid_responses: 0 as f64,
        //             error_responses: 0 as f64,
        //             rejected_payloads: 0 as f64,
        //         },
        //     }
        // }

        todo!()
    }
}
