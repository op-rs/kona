package supervisor

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
)

func TestNetworkConfig(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	t.Require().Equal(out.L2CLA.ChainID().String(), "2151908")
	t.Require().Equal(out.L2CLB.ChainID().String(), "2151909")
	t.Require().Equal(out.L2CLA.Peers().TotalConnected, uint(0))
	t.Require().Equal(out.L2CLB.Peers().TotalConnected, uint(0))
}
