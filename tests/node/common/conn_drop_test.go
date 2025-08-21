package node

import (
	"sync"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	kona_presets "github.com/op-rs/kona/node/presets"
)

// Ensure that kona-nodes reconnect to the sequencer and sync properly when the connection is dropped.
func TestConnDropSync(gt *testing.T) {
	t := devtest.SerialT(gt)

	out := kona_presets.NewMixedOpKona(t)

	nodes := out.L2CLKonaValidatorNodes
	sequencer := out.L2CLSequencerNodes()[0]

	t.Gate().Greater(len(nodes), 1, "expected at least two nodes")

	var wg sync.WaitGroup
	for _, node := range nodes {
		wg.Add(1)
		go func(node *dsl.L2CLNode) {
			defer wg.Done()
			clName := node.Escape().ID().Key()

			node.DisconnectPeer(&sequencer)

			// Wait for 2 minutes
			time.Sleep(2 * time.Minute)

			// Ensure that the node is no longer connected to the sequencer
			seqPeers := sequencer.Peers()
			for _, peer := range seqPeers.Peers {
				t.Require().NotEqual(peer.PeerID, node.PeerInfo().PeerID, "expected node %s to be disconnected from sequencer %s", clName, sequencer.Escape().ID().Key())
			}

			node.ConnectPeer(&sequencer)

			// Check that the node is resyncing with the network
			dsl.CheckAll(t, node.MatchedFn(&sequencer, types.LocalSafe, 50), node.MatchedFn(&sequencer, types.LocalUnsafe, 50))

			// Check that the node is connected to the reference node
			peers := node.Peers()
			t.Require().Greater(len(peers.Peers), 0, "expected at least one peer")

			// Check that there is at least a peer with the same ID as the ref node
			found := false
			for _, peer := range peers.Peers {
				if peer.PeerID == sequencer.PeerInfo().PeerID {
					t.Logf("node %s is connected to reference node %s", clName, sequencer.Escape().ID().Key())
					found = true
					break
				}
			}

			t.Require().True(found, "expected node %s to be connected to reference node %s", clName, sequencer.Escape().ID().Key())
		}(&node)
	}
	wg.Wait()
}
