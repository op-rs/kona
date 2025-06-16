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

func TestL2SafeBlockProgress(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	status := out.L2CLA.SyncStatus()
	block_a := status.SafeL2.Number

	err := wait.For(t.Ctx(), 10*time.Second, func() (bool, error) {
		status := out.L2CLA.SyncStatus()
		block_b := status.SafeL2.Number
		return block_a < block_b, nil
	})
	t.Require().NoError(err)
}

func TestSupervisorProgress(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	//  Checks for heads to advance and also asserts success internally.
	out.Supervisor.WaitForUnsafeHeadToAdvance(out.L2ChainA.ChainID(), 1)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 1, "unsafe", 5)
}

func TestDerivationPipeline(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	l2BlockHead := out.Supervisor.L2HeadBlockID(out.L2ChainA.ChainID(), "local-safe")

	// Get current L1 at which L2 is at and wait for new L1 to be synced in supervisor.
	current_l1_at_l2 := out.L2CLA.SyncStatus().CurrentL1
	out.Supervisor.AwaitMinL1(current_l1_at_l2.Number + 1)
	new_l1 := out.Supervisor.FetchSyncStatus().MinSyncedL1

	t.Require().NotEqual(current_l1_at_l2.Hash, new_l1.Hash)
	t.Require().Greater(new_l1.Number, current_l1_at_l2.Number)

	//  Wait for the L2 chain to sync to the new L1 block.
	err := wait.For(t.Ctx(), 5*time.Second, func() (bool, error) {
		new_l1_at_l2 := out.L2CLA.SyncStatus().CurrentL1
		return new_l1_at_l2.Number >= new_l1.Number, nil
	})
	t.Require().NoError(err)

	new_l2BlockHead := out.Supervisor.L2HeadBlockID(out.L2ChainA.ChainID(), "local-safe")
	t.Require().Greater(new_l2BlockHead.Number, l2BlockHead.Number)
}
