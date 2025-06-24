package supervisor

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
)

const (
	L2A_CHAIN_ID = "2151908"
	L2B_CHAIN_ID = "2151909"
)

func TestNetworkConfig(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	t.Require().Equal(out.L2CLA.ChainID().String(), L2A_CHAIN_ID)
	t.Require().Equal(out.L2CLB.ChainID().String(), L2B_CHAIN_ID)
}
