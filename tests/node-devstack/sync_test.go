package nodedevstack

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
)

func TestL2SafeSync(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewSimpleKona(t)

	opNode := out.L2CLOpNode
	konaNode := out.L2CLKona

	dsl.CheckAll(t, konaNode.ReachedFn(types.LocalSafe, 20, 40), opNode.ReachedFn(types.LocalSafe, 20, 40))

	dsl.CheckAll(t, konaNode.MatchedFn(opNode, types.LocalSafe, 40))
}

func TestL2UnsafeSync(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewSimpleKona(t)

	opNode := out.L2CLOpNode
	konaNode := out.L2CLKona

	dsl.CheckAll(t, konaNode.ReachedFn(types.LocalUnsafe, 40, 40), opNode.ReachedFn(types.LocalUnsafe, 40, 40))

	dsl.CheckAll(t, konaNode.MatchedFn(opNode, types.LocalUnsafe, 40))
}
