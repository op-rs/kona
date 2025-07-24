package node

import (
	"sync"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	"golang.org/x/sync/errgroup"
)

// TestConnDrops tests what happens when a node drops his connection to his peers.
// We simulate that by blacklisting all the peers of a node.
func TestConnDrops(gt *testing.T) {
	t := devtest.SerialT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	ref := nodes[0]

	var wg sync.WaitGroup
	for _, node := range nodes {
		if node == ref {
			continue
		}

		t.Log("testing conn drops for node %s", node.Escape().ID().Key())

		wg.Add(1)
		go func() {
			defer wg.Done()

			// Check that both the safe and unsafe chains are advancing
			dsl.CheckAll(t, node.MatchedFn(&ref, types.LocalSafe, 50), node.MatchedFn(&ref, types.LocalUnsafe, 50))

			// Blacklist all the peers of the node
			peers := node.Peers()
			for _, peer := range peers.Peers {
				t.Log("blacklisting peer %s", peer.PeerID)
				node.Escape().P2PAPI().BlockPeer(t.Ctx(), peer.PeerID)
			}

			// Check that the number of connected peers is correct
			peers, err := node.Escape().P2PAPI().Peers(t.Ctx(), true)
			t.Require().NoError(err, "failed to get peers")
			t.Require().Equal(len(peers.Peers), 0, "expected no peers connected to the node")

			// Wait for the safe chain to advance. The node should _only_ be able to sync the L1 chain: only the safe chain should advance.
			// The local safe chain may diverge from the reference node, but the unsafe chain should be in sync.
			dsl.CheckAll(t, node.AdvancedFn(types.LocalSafe, 20, 50), node.NotAdvancedFn(types.LocalUnsafe, 50))

			// Unblock the peers of the node
			for _, peer := range peers.Peers {
				t.Log("unblocking peer %s", peer.PeerID)
				node.Escape().P2PAPI().UnblockPeer(t.Ctx(), peer.PeerID)
			}

			// Wait for the safe and unsafe chains to advance. The node should be able to sync both the safe and unsafe chains.
			// The chains should be in sync with the reference node!
			dsl.CheckAll(t, node.MatchedFn(&ref, types.LocalSafe, 50), node.MatchedFn(&ref, types.LocalUnsafe, 50))

		}()
	}

	wg.Wait()
}

// TestConnDropsWithSequencer tests what happens when the sequencer node drops his connection to all the other nodes of the network.
// In that case, the sequencer should be able to sync both the safe and unsafe chains. The other nodes should be able to sync the L1 chain but diverge from the sequencer.
func TestConnDropsWithSequencer(gt *testing.T) {
	t := devtest.SerialT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	sequencerList := filterSequencer(nodes)

	// Ensure that there is only one sequencer node (otherwise op-conductor might make matters tricky)
	t.Gate().Equal(len(sequencerList), 1, "expected only one sequencer node")

	sequencer := sequencerList[0]

	// Blacklist all the peers of the sequencer
	peers := sequencer.Peers()
	for _, peer := range peers.Peers {
		t.Log("blacklisting peer %s", peer.PeerID)
		sequencer.Escape().P2PAPI().BlockPeer(t.Ctx(), peer.PeerID)
	}

	// Check that the number of connected peers is correct
	peers, err := sequencer.Escape().P2PAPI().Peers(t.Ctx(), true)
	t.Require().NoError(err, "failed to get peers")
	t.Require().Equal(len(peers.Peers), 0, "expected no peers connected to the sequencer")

	// Now:
	// - The sequencer should be able to sync the L1 chain
	// - The other nodes should be able to sync the L1 chain but diverge from the sequencer
	// - The sequencer should be able to sync the safe and unsafe chains
	// - The other nodes should be able to sync the safe and unsafe chains

	toCheck := []dsl.CheckFunc{}
	toCheckErr := []dsl.CheckFunc{}

	toCheck = append(toCheck, sequencer.AdvancedFn(types.LocalSafe, 20, 50), sequencer.AdvancedFn(types.LocalUnsafe, 20, 50))

	for _, node := range nodes {
		if node == sequencer {
			continue
		}

		toCheck = append(toCheck, node.AdvancedFn(types.LocalSafe, 20, 50))
		toCheck = append(toCheck, node.NotAdvancedFn(types.LocalUnsafe, 50))

		// The other nodes should _always_ diverge from the sequencer
		toCheckErr = append(toCheckErr, node.MatchedFn(&sequencer, types.LocalSafe, 50))
	}

	dsl.CheckAll(t, toCheck...)
	CheckErr(t, toCheckErr...)

	// Unblock the peers of the sequencer. The network should get back to normal.
	for _, peer := range peers.Peers {
		t.Log("unblocking peer %s", peer.PeerID)
		sequencer.Escape().P2PAPI().UnblockPeer(t.Ctx(), peer.PeerID)
	}

	toCheck = []dsl.CheckFunc{}

	// Wait for the safe and unsafe chains to advance. The sequencer should be able to sync both the safe and unsafe chains.
	toCheck = append(toCheck, sequencer.AdvancedFn(types.LocalSafe, 20, 50), sequencer.AdvancedFn(types.LocalUnsafe, 20, 50))

	for _, node := range nodes {
		if node == sequencer {
			continue
		}

		toCheck = append(toCheck, node.MatchedFn(&sequencer, types.LocalSafe, 50))
		toCheck = append(toCheck, node.MatchedFn(&sequencer, types.LocalUnsafe, 50))
	}

	dsl.CheckAll(t, toCheck...)
	CheckErr(t, toCheckErr...)
}

// Like CheckAll, but expects an error.
func CheckErr(t devtest.T, checks ...dsl.CheckFunc) {
	var g errgroup.Group
	for _, check := range checks {
		check := check
		g.Go(func() error {
			return check()
		})
	}
	t.Require().Error(g.Wait())
}

// TestConnDropsEngineTaskCount tests that the engine task count is correctly updated when a node drops his connection to his peers.
func TestConnDropsEngineTaskCount(gt *testing.T) {
	t := devtest.SerialT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	ref := nodes[0]

	var wg sync.WaitGroup
	for _, node := range nodes {
		if node == ref {
			continue
		}

		t.Log("testing conn drops for node %s", node.Escape().ID().Key())

		wg.Add(1)
		go func() {
			defer wg.Done()

			// Blacklist all the peers of the node
			peers := node.Peers()
			for _, peer := range peers.Peers {
				t.Log("blacklisting peer %s", peer.PeerID)
				node.Escape().P2PAPI().BlockPeer(t.Ctx(), peer.PeerID)
			}

			// Check that the number of connected peers is correct
			peers, err := node.Escape().P2PAPI().Peers(t.Ctx(), true)
			t.Require().NoError(err, "failed to get peers")
			t.Require().Equal(len(peers.Peers), 0, "expected no peers connected to the node")

			// Check that the engine task count is correct
			clRPC, err := GetNodeRPCEndpoint(t.Ctx(), &node)
			t.Require().NoError(err, "failed to get RPC endpoint for node %s", node.Escape().ID().Key())
			wsRPC := websocketRPC(clRPC)

			const SECS_WAIT_FOR_ENGINE = 10
			queue := GetDevWS(t, wsRPC, "engine_queue_size", time.After(SECS_WAIT_FOR_ENGINE*time.Second))

			// Check that the engine task count is correct
			for _, q := range queue {
				t.Require().LessOrEqual(q, 1, "expected at most 1 engine task")
			}

			// Unblock the peers of the node
			for _, peer := range peers.Peers {
				t.Log("unblocking peer %s", peer.PeerID)
				node.Escape().P2PAPI().UnblockPeer(t.Ctx(), peer.PeerID)
			}

			// Wait for the safe and unsafe chains to advance. The node should be able to sync both the safe and unsafe chains.
			// The chains should be in sync with the reference node!
			dsl.CheckAll(t, node.MatchedFn(&ref, types.LocalSafe, 50), node.MatchedFn(&ref, types.LocalUnsafe, 50))

		}()
	}

	wg.Wait()

}
