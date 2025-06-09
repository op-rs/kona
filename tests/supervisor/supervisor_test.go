package supervisor

import (
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-e2e/e2eutils/wait"
)

func TestNetworkConfig(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	t.Require().Equal(out.L2CLA.ChainID().String(), "2151908")
	t.Require().Equal(out.L2CLB.ChainID().String(), "2151909")
	t.Require().Equal(out.L2CLA.Peers().TotalConnected, uint(0))
	t.Require().Equal(out.L2CLB.Peers().TotalConnected, uint(0))
}

func TestL2UnsafeBlockProgress(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	status := out.L2CLA.SyncStatus()
	block_a := status.UnsafeL2.Number

	err := wait.For(t.Ctx(), 2*time.Second, func() (bool, error) {
		status := out.L2CLA.SyncStatus()
		block_b := status.UnsafeL2.Number
		return block_a < block_b, nil
	})
	t.Require().NoError(err)
}

func TestSupervisorProgress(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	out.Supervisor.AdvancedUnsafeHead(out.L2CLA.ChainID(), 3)
	out.Supervisor.AdvancedSafeHead(out.L2ChainA.ChainID(), 1, 5)
}
