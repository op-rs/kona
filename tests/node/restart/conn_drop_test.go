package node_restart

import (
	"fmt"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-service/retry"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	node_utils "github.com/op-rs/kona/node/utils"
)

// Ensure that kona-nodes reconnect to the sequencer and sync properly when the connection is dropped.
func TestConnDropSync(gt *testing.T) {
	t := devtest.SerialT(gt)

	out := node_utils.NewMixedOpKona(t)

	nodes := out.L2CLValidatorNodes()
	sequencerNodes := out.L2CLSequencerNodes()
	t.Gate().Greater(len(nodes), 0, "expected at least one validator node")
	t.Gate().Greater(len(sequencerNodes), 0, "expected at least one sequencer node")

	sequencer := sequencerNodes[0]

	var postDisconnectCheckFuns []dsl.CheckFunc
	for _, node := range nodes {
		clName := node.Escape().ID().Key()

		node.DisconnectPeer(&sequencer)

		// Ensure that the node is no longer connected to the sequencer
		seqPeers := sequencer.Peers()
		for _, peer := range seqPeers.Peers {
			t.Require().NotEqual(peer.PeerID, node.PeerInfo().PeerID, "expected node %s to be disconnected from sequencer %s", clName, sequencer.Escape().ID().Key())
		}

		// Check that...
		// - the node's safe head is advancing
		// - the node's unsafe head is advancing (through consolidation)
		// - the node's safe head's number is catching up with the unsafe head's number
		// - the node's unsafe head is strictly lagging behind the sequencer's unsafe head
		postDisconnectCheckFuns = append(postDisconnectCheckFuns, node.AdvancedFn(types.LocalSafe, 50, 200), node.AdvancedFn(types.LocalUnsafe, 50, 200), SafeUnsafeMatchedFn(t, node, 50))
	}

	postDisconnectCheckFuns = append(postDisconnectCheckFuns, sequencer.AdvancedFn(types.LocalUnsafe, 50, 200))

	dsl.CheckAll(t, postDisconnectCheckFuns...)

	var postReconnectCheckFuns []dsl.CheckFunc
	for _, node := range nodes {
		clName := node.Escape().ID().Key()

		node.ConnectPeer(&sequencer)

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

		// Check that the node is resyncing with the unsafe head network
		postReconnectCheckFuns = append(postReconnectCheckFuns, node.MatchedFn(&sequencer, types.LocalSafe, 50), node.MatchedFn(&sequencer, types.LocalUnsafe, 50))
	}

	dsl.CheckAll(t, postReconnectCheckFuns...)
}

// MatchedFn returns a lambda that checks the baseNode head with given safety level is matched with the refNode chain sync status provider
// Composable with other lambdas to wait in parallel
func SafeUnsafeMatchedFn(t devtest.T, clNode dsl.L2CLNode, attempts int) dsl.CheckFunc {
	logger := t.Logger()
	chainID := clNode.ChainID()
	return func() error {
		return retry.Do0(t.Ctx(), attempts, &retry.FixedStrategy{Dur: 2 * time.Second},
			func() error {
				base := clNode.ChainSyncStatus(chainID, types.LocalSafe)
				ref := clNode.ChainSyncStatus(chainID, types.LocalUnsafe)
				if ref.Hash == base.Hash && ref.Number == base.Number {
					logger.Info("Node safe and unsafe heads matched", "ref", ref.Number, "base", base.Number)
					return nil
				}
				logger.Info("Node safe and unsafe heads not matched", "safe", base.Number, "unsafe", ref.Number, "ref", ref.Hash, "base", base.Hash)
				return fmt.Errorf("expected safe and unsafe heads to match")
			})
	}
}
