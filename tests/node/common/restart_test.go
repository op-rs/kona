package node

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	kona_presets "github.com/op-rs/kona/node/presets"
)

// Ensure that kona-nodes reconnect to the sequencer and sync properly when the connection is dropped.
func TestRestartSync(gt *testing.T) {
	t := devtest.SerialT(gt)

	out := kona_presets.NewMixedOpKona(t)

	nodes := out.L2CLKonaValidatorNodes
	sequencerNodes := out.L2CLSequencerNodes()
	t.Gate().Greater(len(nodes), 0, "expected at least one validator node")
	t.Gate().Greater(len(sequencerNodes), 0, "expected at least one sequencer node")

	sequencer := sequencerNodes[0]

	var wg sync.WaitGroup
	for _, node := range nodes {
		t.Logf("testing restarts for node %s", node.Escape().ID().Key())
		wg.Add(1)
		go func(node *dsl.L2CLNode) {
			defer wg.Done()
			clName := node.Escape().ID().Key()

			t.Logf("stopping node %s", clName)
			node.Stop()

			// Ensure that the node is no longer connected to the sequencer
			seqPeers := sequencer.Peers()
			for _, peer := range seqPeers.Peers {
				t.Require().NotEqual(peer.PeerID, node.PeerInfo().PeerID, "expected node %s to be disconnected from sequencer %s", clName, sequencer.Escape().ID().Key())
			}

			// Ensure that the node is stopped
			// Check that calling any rpc method returns an error
			rpc := GetNodeRPCEndpoint(node)
			var out *eth.SyncStatus
			err := rpc.CallContext(context.Background(), &out, "opp2p_syncStatus")
			t.Require().Error(err, "expected node %s to be stopped", clName)

			time.Sleep(2 * time.Minute)

			t.Logf("starting node %s", clName)
			node.Start()

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
