package nodedevstack

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
)

// Check that all the nodes in the network are synced to the local safe block and can catch up to the sequencer node.
func TestL2SafeSync(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

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

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	checkFuns := make([]dsl.CheckFunc, 0, 2*len(nodes))

	for _, node := range nodes {
		checkFuns = append(checkFuns, node.ReachedFn(types.LocalUnsafe, 40, 40))
		checkFuns = append(checkFuns, node.MatchedFn(&nodes[0], types.LocalUnsafe, 40))
	}

	dsl.CheckAll(t, checkFuns...)
}
