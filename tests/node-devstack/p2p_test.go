package nodedevstack

import (
	"fmt"
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-service/apis"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	"github.com/libp2p/go-libp2p/core/network"
	"github.com/stretchr/testify/require"
)

func checkProtocols(t devtest.T, peer *apis.PeerInfo, nodeName string) {
	require.Contains(t, peer.Protocols, "/meshsub/1.0.0", fmt.Sprintf("%s is not using the meshsub protocol 1.0.0", nodeName))
	require.Contains(t, peer.Protocols, "/meshsub/1.1.0", fmt.Sprintf("%s is not using the meshsub protocol 1.1.0", nodeName))
	require.Contains(t, peer.Protocols, "/meshsub/1.2.0", fmt.Sprintf("%s is not using the meshsub protocol 1.2.0", nodeName))
	require.Contains(t, peer.Protocols, "/ipfs/id/1.0.0", fmt.Sprintf("%s is not using the id protocol 1.0.0", nodeName))
	require.Contains(t, peer.Protocols, "/ipfs/id/push/1.0.0", fmt.Sprintf("%s is not using the id push protocol 1.0.0", nodeName))
	require.Contains(t, peer.Protocols, "/floodsub/1.0.0", fmt.Sprintf("%s is not using the floodsub protocol 1.0.0", nodeName))

}

func checkPeerStats(t devtest.T, peerStats *apis.PeerStats, nodeName string) {
	require.GreaterOrEqual(t, peerStats.Connected, uint(1), fmt.Sprintf("%s has no connected peers", nodeName))
	require.Greater(t, peerStats.Table, uint(0), fmt.Sprintf("%s has no peers in the discovery table", nodeName))
	require.GreaterOrEqual(t, peerStats.BlocksTopic, uint(1), fmt.Sprintf("%s has no peers in the blocks topic", nodeName))
	require.GreaterOrEqual(t, peerStats.BlocksTopicV2, uint(1), fmt.Sprintf("%s has no peers in the blocks topic v2", nodeName))
	require.GreaterOrEqual(t, peerStats.BlocksTopicV3, uint(1), fmt.Sprintf("%s has no peers in the blocks topic v3", nodeName))
	require.GreaterOrEqual(t, peerStats.BlocksTopicV4, uint(1), fmt.Sprintf("%s has no peers in the blocks topic v4", nodeName))
}

func TestP2P(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	opNode := out.L2CLOpNodes[0]
	konaNode := out.L2CLKonaNodes[0]

	// Wait for a few blocks to be produced.
	dsl.CheckAll(t, konaNode.ReachedFn(types.LocalUnsafe, 40, 40), opNode.ReachedFn(types.LocalUnsafe, 40, 40))

	opNodeId := opNode.PeerInfo().PeerID
	konaNodeId := konaNode.PeerInfo().PeerID

	opNodePeers := opNode.Peers()

	found := false
	for _, peer := range opNodePeers.Peers {
		if peer.PeerID == konaNodeId {
			require.Equal(t, peer.Connectedness, network.Connected, "kona node is not connected to the op node")
			checkProtocols(t, peer, "op node")
			found = true
		}
	}

	require.True(t, found, "kona node is not in the op node's peers")

	konaNodePeers := konaNode.Peers()

	found = false
	for _, peer := range konaNodePeers.Peers {
		if peer.PeerID == opNodeId {
			require.Equal(t, peer.Connectedness, network.Connected, "op node is not connected to the kona node")
			checkProtocols(t, peer, "kona node")
			found = true
		}
	}
	require.True(t, found, "op node is not in the kona node's peers")

	opNodePeerStats, err := opNode.Escape().P2PAPI().PeerStats(t.Ctx())
	require.NoError(t, err)
	checkPeerStats(t, opNodePeerStats, "op node")

	konaNodePeerStats, err := konaNode.Escape().P2PAPI().PeerStats(t.Ctx())
	require.NoError(t, err)
	checkPeerStats(t, konaNodePeerStats, "kona node")
}
