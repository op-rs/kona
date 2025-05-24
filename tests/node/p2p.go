package node

import (
	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/op-service/apis"
	"github.com/stretchr/testify/require"
)

// Really simple system test that just checks that each node has at least 1 peer that is connected to its topics.
func peerCount(minPeersKnown uint, minPeersConnected uint) systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		for _, l2 := range l2s {

			for _, node := range l2.Nodes() {
				clRPC := node.CLRPC()
				clName := node.CLName()

				peerStats := apis.PeerStats{}
				err := SendRPCRequest(clRPC, "opp2p_peerStats", &peerStats)

				if err != nil {
					t.Errorf("failed to send RPC request to node %s: %s", clName, err)
				}

				require.GreaterOrEqual(t, peerStats.Known, minPeersKnown, "node %s has not enough known peers", clName)
				require.GreaterOrEqual(t, peerStats.Table, minPeersKnown, "node %s doesn't have enough peers in the discovery table.", clName)

				require.GreaterOrEqual(t, peerStats.Connected, minPeersConnected, "node %s doesn't have enough peers connected.", clName)
				require.GreaterOrEqual(t, peerStats.BlocksTopicV4, minPeersConnected, "node %s doesn't have enough blocks topic v4 peers.", clName)
				require.GreaterOrEqual(t, peerStats.BlocksTopicV3, minPeersConnected, "node %s doesn't have enough blocks topic v3 peers.", clName)
				require.GreaterOrEqual(t, peerStats.BlocksTopicV2, minPeersConnected, "node %s doesn't have enough blocks topic v2 peers.", clName)
				require.GreaterOrEqual(t, peerStats.BlocksTopic, minPeersConnected, "node %s doesn't have enough blocks topic peers.", clName)
			}
		}
	}
}

// Ensure that all the connected peers's addresses are part of the network.
func allPeersInNetwork() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()

		for _, l2 := range l2s {
			peerIds := make(map[string]bool)
			nodeIds := make(map[string]bool)

			// Get all the node addresses that are connected to the network.
			for _, node := range l2.Nodes() {
				clRPC := node.CLRPC()
				clName := node.CLName()

				peerInfo := apis.PeerInfo{}
				err := SendRPCRequest(clRPC, "opp2p_self", &peerInfo)

				if err != nil {
					t.Errorf("failed to send RPC request to node %s: %s", clName, err)
				}

				peerIds[peerInfo.PeerID.String()] = true
				nodeIds[peerInfo.NodeID.String()] = true
			}

			for _, node := range l2.Nodes() {
				clRPC := node.CLRPC()
				clName := node.CLName()

				peerDump := apis.PeerDump{}
				err := SendRPCRequest(clRPC, "opp2p_peers", &peerDump, true)

				if err != nil {
					t.Errorf("failed to send RPC request to node %s: %s", clName, err)
				}

				for _, peer := range peerDump.Peers {
					_, ok := peerIds[peer.PeerID.String()]
					require.True(t, ok, "node %s is connected to an unknown peer %s", clName, peer.PeerID)

					_, ok = nodeIds[peer.NodeID.String()]
					require.True(t, ok, "node %s is connected to an unknown node %s", clName, peer.NodeID)
				}

			}
		}
	}
}
