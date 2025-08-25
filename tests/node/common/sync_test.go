package node

import (
	"strings"
	"sync"
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	node_utils "github.com/op-rs/kona/node/utils"
)

// Check that all the nodes in the network are synced to the local safe block and can catch up to the sequencer node.
func TestL2SafeSync(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := node_utils.NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	checkFuns := make([]dsl.CheckFunc, 0, 2*len(nodes))

	for _, node := range nodes {
		checkFuns = append(checkFuns, node.ReachedFn(types.LocalSafe, 20, 40))
		checkFuns = append(checkFuns, node.MatchedFn(&nodes[0], types.LocalSafe, 40))
	}

	dsl.CheckAll(t, checkFuns...)
}

// Check that all the nodes in the network are synced to the local unsafe block and can catch up to the sequencer node.
func TestL2UnsafeSync(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := node_utils.NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	checkFuns := make([]dsl.CheckFunc, 0, 2*len(nodes))

	for _, node := range nodes {
		checkFuns = append(checkFuns, node.ReachedFn(types.LocalUnsafe, 40, 40))
		checkFuns = append(checkFuns, node.MatchedFn(&nodes[0], types.LocalUnsafe, 40))
	}

	dsl.CheckAll(t, checkFuns...)
}

// Check that all the kona nodes in the network are synced to the finalized block.
func TestL2FinalizedSync(gt *testing.T) {
	t := devtest.ParallelT(gt)
	t.Skip("Skipping finalized sync test")

	out := node_utils.NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	checkFuns := make([]dsl.CheckFunc, 0, 2*len(nodes))

	for _, node := range nodes {
		checkFuns = append(checkFuns, node.ReachedFn(types.Finalized, 10, 600))
	}

	dsl.CheckAll(t, checkFuns...)
}

func isSequencer(node *dsl.L2CLNode) bool {
	return strings.Contains(node.Escape().ID().Key(), string(node_utils.Sequencer))
}

func filterSequencer(nodes []dsl.L2CLNode) []dsl.L2CLNode {
	out := make([]dsl.L2CLNode, 0, len(nodes))
	for _, node := range nodes {
		if isSequencer(&node) {
			out = append(out, node)
		}
	}
	return out
}

func TestSyncWithSequencer(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := node_utils.NewMixedOpKona(t)

	nodes := out.L2CLValidatorNodes()

	// Find the sequencer nodes.
	sequencers := filterSequencer(nodes)
	t.Gate().Equal(len(sequencers), 1, "expected exactly one sequencer")
	sequencer := sequencers[0]

	// Check that all the nodes are lagging behind the sequencer for the local unsafe head.
	var wg sync.WaitGroup
	for _, node := range nodes {
		wg.Add(1)
		go func(node *dsl.L2CLNode) {
			defer wg.Done()
			node.Lagged(&sequencer, types.LocalUnsafe, 40, true)
		}(&node)
	}
	wg.Wait()
}
