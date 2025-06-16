package nodedevstack

import (
	"fmt"
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-service/apis"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	"github.com/libp2p/go-libp2p/core/network"
	"github.com/libp2p/go-libp2p/core/peer"
	"github.com/stretchr/testify/require"
)

func checkProtocols(t devtest.T, peer *apis.PeerInfo) {
	nodeName := peer.PeerID.String()

	require.Contains(t, peer.Protocols, "/meshsub/1.0.0", fmt.Sprintf("%s is not using the meshsub protocol 1.0.0", nodeName))
	require.Contains(t, peer.Protocols, "/meshsub/1.1.0", fmt.Sprintf("%s is not using the meshsub protocol 1.1.0", nodeName))
	require.Contains(t, peer.Protocols, "/meshsub/1.2.0", fmt.Sprintf("%s is not using the meshsub protocol 1.2.0", nodeName))
	require.Contains(t, peer.Protocols, "/ipfs/id/1.0.0", fmt.Sprintf("%s is not using the id protocol 1.0.0", nodeName))
	require.Contains(t, peer.Protocols, "/ipfs/id/push/1.0.0", fmt.Sprintf("%s is not using the id push protocol 1.0.0", nodeName))
	require.Contains(t, peer.Protocols, "/floodsub/1.0.0", fmt.Sprintf("%s is not using the floodsub protocol 1.0.0", nodeName))

}

// Check that the node has enough connected peers and peers in the discovery table.
func checkPeerStats(t devtest.T, node *dsl.L2CLNode, minConnected uint, minTable uint, minBlocksTopic uint) {
	peerStats, err := node.Escape().P2PAPI().PeerStats(t.Ctx())
	nodeName := node.Escape().ID()

	require.NoError(t, err, "failed to get peer stats for %s", nodeName)

	require.GreaterOrEqual(t, peerStats.Connected, minConnected, fmt.Sprintf("%s has no connected peers", nodeName))
	require.Greater(t, peerStats.Table, minTable, fmt.Sprintf("%s has no peers in the discovery table", nodeName))
	require.GreaterOrEqual(t, peerStats.BlocksTopic, minBlocksTopic, fmt.Sprintf("%s has no peers in the blocks topic", nodeName))
	require.GreaterOrEqual(t, peerStats.BlocksTopicV2, minBlocksTopic, fmt.Sprintf("%s has no peers in the blocks topic v2", nodeName))
	require.GreaterOrEqual(t, peerStats.BlocksTopicV3, minBlocksTopic, fmt.Sprintf("%s has no peers in the blocks topic v3", nodeName))
	require.GreaterOrEqual(t, peerStats.BlocksTopicV4, minBlocksTopic, fmt.Sprintf("%s has no peers in the blocks topic v4", nodeName))
}

// Check that `node` is connected to the other node and exposes the expected protocols.
func arePeers(t devtest.T, node *dsl.L2CLNode, otherNodeId peer.ID) {
	nodePeers := node.Peers()

	found := false
	for _, peer := range nodePeers.Peers {
		if peer.PeerID == otherNodeId {
			require.Equal(t, peer.Connectedness, network.Connected, fmt.Sprintf("%s is not connected to the %s", node.Escape().ID(), otherNodeId))
			checkProtocols(t, peer)
			found = true
		}
	}
	require.True(t, found, fmt.Sprintf("%s is not in the %s's peers", otherNodeId, node.Escape().ID()))
}

func TestP2PMinimal(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	opNode := out.L2CLOpNodes[0]
	konaNode := out.L2CLKonaNodes[0]

	opNodeId := opNode.PeerInfo().PeerID
	konaNodeId := konaNode.PeerInfo().PeerID

	// Wait for a few blocks to be produced.
	dsl.CheckAll(t, konaNode.ReachedFn(types.LocalUnsafe, 40, 40), opNode.ReachedFn(types.LocalUnsafe, 40, 40))

	// Check that the nodes are connected to each other.
	arePeers(t, &opNode, konaNodeId)
	arePeers(t, &konaNode, opNodeId)

	// Check that the nodes have enough connected peers and peers in the discovery table.
	checkPeerStats(t, &opNode, 1, 1, 1)
	checkPeerStats(t, &konaNode, 1, 1, 1)
}

// Check that, for every node in the network, all the peers are connected to the expected protocols and the same chainID.
func TestP2PProtocols(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	for _, node := range nodes {
		for _, peer := range node.Peers().Peers {
			checkProtocols(t, peer)
		}
	}
}

func TestP2PChainID(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLKonaNodes
	chainID := nodes[0].PeerInfo().ChainID

	for _, node := range nodes {
		nodeChainID, ok := node.Escape().ID().ChainID().Uint64()
		require.True(t, ok, "chainID is too large for a uint64")
		require.Equal(t, chainID, nodeChainID, fmt.Sprintf("%s has a different chainID", node.Escape().ID()))

		for _, peer := range node.Peers().Peers {
			require.Equal(t, chainID, peer.ChainID, fmt.Sprintf("%s has a different chainID", node.Escape().ID()))
		}
	}
}

// Check that, for all the nodes in the network, all the peers are connected to a known node.
func TestP2PPeersInNetwork(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	nodeIds := make([]string, 0, len(nodes))
	for _, node := range nodes {
		nodeIds = append(nodeIds, node.PeerInfo().PeerID.String())
	}

	for _, node := range nodes {
		for _, peer := range node.Peers().Peers {
			require.Contains(t, nodeIds, peer.PeerID.String(), fmt.Sprintf("%s has a peer that is not in the network: %s", node.Escape().ID(), peer.PeerID))
		}
	}
}

// Check that all the nodes in the network have enough connected peers and peers in the discovery table.
func TestNetworkConnectivity(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()
	numNodes := len(nodes)

	for _, node := range nodes {
		checkPeerStats(t, &node, uint(numNodes)-1, uint(numNodes)/2, uint(numNodes)/2)
	}
}
