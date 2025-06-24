package node

import (
	"sync"
	"testing"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/op-service/apis"
	"github.com/libp2p/go-libp2p/core/network"
	"github.com/stretchr/testify/require"
)

// Check that the node p2p RPC endpoints are working.
func TestSystemP2PPeers(t *testing.T) {
	t.Skip("TODO(@theochap): this test is broken because the devnet-sdk doesn't support the newer versions of the optimism-package.")

	systest.SystemTest(t,
		p2pPeersAndPeerStats(),
	)

	systest.SystemTest(t,
		p2pSelfAndPeers(),
	)

	systest.SystemTest(t,
		p2pBanPeer(),
	)
}

// Ensure that the `opp2p_peers` and `opp2p_self` RPC endpoints return the same information.
func p2pSelfAndPeers() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		var wg sync.WaitGroup
		for _, l2 := range l2s {
			for _, node := range l2.Nodes() {
				wg.Add(1)
				go func(l2 system.L2Chain, node system.Node) {
					defer wg.Done()
					clRPC := node.CLRPC()
					clName := node.CLName()

					if !isKonaNode(t, clRPC, clName) {
						t.Log("is not a kona node, skipping test", clName)
						return
					}

					// Gather the peers for the node.
					peers := &apis.PeerDump{}
					require.NoError(t, SendRPCRequest(clRPC, "opp2p_peers", peers, true), "failed to send RPC request to node %s: %s", clName)

					// Check that every peer's info matches the node's info.
					for _, peer := range peers.Peers {
						// Find the node that is the peer. We loop over all the nodes in the network and try to match their peerID's to
						// the peerID we are looking for.
						for _, node := range l2.Nodes() {
							// We get the peer's info.
							otherPeerInfo := &apis.PeerInfo{}
							require.NoError(t, SendRPCRequest(node.CLRPC(), "opp2p_self", otherPeerInfo), "failed to send RPC request to node %s: %s", clName)

							// These checks fail for the op-node. It seems that their p2p handler is flaky and doesn't always return the correct peer info.
							if otherPeerInfo.PeerID == peer.PeerID && isKonaNode(t, node.CLRPC(), node.CLName()) {
								require.Equal(t, otherPeerInfo.NodeID, peer.NodeID, "nodeID mismatch, %s", node.CLName())
								require.Equal(t, otherPeerInfo.ProtocolVersion, peer.ProtocolVersion, "protocolVersion mismatch, %s", node.CLName())

								// Sometimes the node is not part of the discovery table so we don't have an ENR.
								if peer.ENR != "" {
									require.Equal(t, otherPeerInfo.ENR, peer.ENR, "ENR mismatch, %s", node.CLName())
								}

								// Sometimes the node is not part of the discovery table so we don't have a valid chainID.
								if peer.ChainID != 0 {
									require.Equal(t, otherPeerInfo.ChainID, peer.ChainID, "chainID mismatch, %s", node.CLName())
								}

								for _, addr := range peer.Addresses {
									require.Contains(t, otherPeerInfo.Addresses, addr, "the peer's address should be in the node's known addresses, %s", node.CLName())
								}

								for _, protocol := range peer.Protocols {
									require.Contains(t, otherPeerInfo.Protocols, protocol, "protocol %s not found, %s", protocol, node.CLName())
								}

								require.Equal(t, otherPeerInfo.UserAgent, peer.UserAgent, "userAgent mismatch, %s", node.CLName())
							}
						}
					}
				}(l2, node)
			}
		}
		wg.Wait()
	}
}

// Check that the `opp2p_peers` and `opp2p_peerStats` RPC endpoints return coherent information.
func p2pPeersAndPeerStats() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		var wg sync.WaitGroup
		for _, l2 := range l2s {
			for _, node := range l2.Nodes() {
				wg.Add(1)
				go func(l2 system.L2Chain, node system.Node) {
					defer wg.Done()
					clRPC := node.CLRPC()
					clName := node.CLName()

					if !isKonaNode(t, clRPC, clName) {
						t.Log("is not a kona node, skipping test", clName)
						return
					}

					peers := &apis.PeerDump{}
					require.NoError(t, SendRPCRequest(clRPC, "opp2p_peers", peers, true), "failed to send RPC request to node %s: %s", clName)

					peerStats := &apis.PeerStats{}
					require.NoError(t, SendRPCRequest(clRPC, "opp2p_peerStats", peerStats), "failed to send RPC request to node %s: %s", clName)

					require.Equal(t, peers.TotalConnected, peerStats.Connected, "totalConnected mismatch node %s", clName)
					require.Equal(t, len(peers.Peers), int(peers.TotalConnected), "peer count mismatch node %s", clName)
				}(l2, node)
				wg.Wait()
			}
		}
	}
}

func p2pBanPeer() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		for _, l2 := range l2s {
			for _, node := range l2.Nodes() {
				clRPC := node.CLRPC()
				clName := node.CLName()

				if !isKonaNode(t, clRPC, clName) {
					t.Log("is not a kona node, skipping test", clName)
					return
				}

				peers := &apis.PeerDump{}
				require.NoError(t, SendRPCRequest(clRPC, "opp2p_peers", peers, true), "failed to send RPC request to node %s: %s", clName)

				connectedPeers := peers.TotalConnected

				// Try to ban a peer.
				// We pick the first peer that is connected.
				peerToBan := ""
				for _, peer := range peers.Peers {
					if peer.Connectedness == network.Connected {
						peerToBan = peer.PeerID.String()
						break
					}
				}

				require.NotEmpty(t, peerToBan, "no connected peer found")

				require.NoError(t, SendRPCRequest[any](clRPC, "opp2p_blockPeer", nil, peerToBan), "failed to send RPC request to node %s: %s", clName)

				// Check that the peer is banned.
				peersAfterBan := &apis.PeerDump{}
				require.NoError(t, SendRPCRequest(clRPC, "opp2p_peers", peersAfterBan, true), "failed to send RPC request to node %s: %s", clName)

				require.Equal(t, connectedPeers, peersAfterBan.TotalConnected, "totalConnected mismatch node %s", clName)

				contains := false
				// Loop over all the banned peers and check that the peer is banned.
				for _, bannedPeer := range peersAfterBan.BannedPeers {
					if bannedPeer.String() == peerToBan {
						require.Equal(t, bannedPeer.String(), peerToBan, "peer %s not banned", peerToBan)
						contains = true
					}
				}

				require.True(t, contains, "peer %s not banned", peerToBan)

				// Try to unban the peer.
				require.NoError(t, SendRPCRequest[any](clRPC, "opp2p_unblockPeer", nil, peerToBan), "failed to send RPC request to node %s: %s", clName)

				// Check that the peer is unbanned.
				peersAfterUnban := &apis.PeerDump{}
				require.NoError(t, SendRPCRequest(clRPC, "opp2p_peers", peersAfterUnban, true), "failed to send RPC request to node %s: %s", clName)

				require.Equal(t, connectedPeers, peersAfterUnban.TotalConnected, "totalConnected mismatch node %s", clName)
				require.NotContains(t, peersAfterUnban.BannedPeers, peerToBan, "peer %s is banned", peerToBan)
			}
		}
	}
}
