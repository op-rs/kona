package supervisor

import (
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-e2e/e2eutils/wait"
)

const (
	// UnSafeHeadAdvanceRetries is the number of retries for unsafe head advancement
	UnSafeHeadAdvanceRetries = 15

	// CrossUnsafeHeadAdvanceRetries is the number of retries for cross-unsafe head advancement
	CrossUnsafeHeadAdvanceRetries = 15

	// LocalSafeHeadAdvanceRetries is the number of retries for safe head advancement
	LocalSafeHeadAdvanceRetries = 15

	// SafeHeadAdvanceRetries is the number of retries for safe head advancement
	SafeHeadAdvanceRetries = 25

	// FinalizedHeadAdvanceRetries is the number of retries for finalized head advancement
	FinalizedHeadAdvanceRetries = 100
)

func TestSupervisorLocalUnsafeHeadAdvancing(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	l2aChainID := out.L2CLA.ChainID()
	l2bChainID := out.L2CLB.ChainID()

	supervisorStatus := out.Supervisor.FetchSyncStatus()

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 2, "unsafe", UnSafeHeadAdvanceRetries)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 2, "unsafe", UnSafeHeadAdvanceRetries)

	err := wait.For(t.Ctx(), 5*time.Second, func() (bool, error) {
		latestSupervisorStatus := out.Supervisor.FetchSyncStatus()
		return latestSupervisorStatus.Chains[l2aChainID].LocalUnsafe.Number > supervisorStatus.Chains[l2aChainID].LocalUnsafe.Number &&
			latestSupervisorStatus.Chains[l2bChainID].LocalUnsafe.Number >= supervisorStatus.Chains[l2bChainID].LocalUnsafe.Number, nil
	})
	t.Require().NoError(err)
}

func TestSupervisorCrossUnsafeHeadAdvancing(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	l2aChainID := out.L2CLA.ChainID()
	l2bChainID := out.L2CLB.ChainID()

	supervisorStatus := out.Supervisor.FetchSyncStatus()

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 2, "cross-unsafe", CrossUnsafeHeadAdvanceRetries)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 2, "cross-unsafe", CrossUnsafeHeadAdvanceRetries)

	err := wait.For(t.Ctx(), 5*time.Second, func() (bool, error) {
		latestSupervisorStatus := out.Supervisor.FetchSyncStatus()
		return latestSupervisorStatus.Chains[l2aChainID].LocalUnsafe.Number > supervisorStatus.Chains[l2aChainID].LocalUnsafe.Number &&
			latestSupervisorStatus.Chains[l2bChainID].LocalUnsafe.Number >= supervisorStatus.Chains[l2bChainID].LocalUnsafe.Number, nil
	})
	t.Require().NoError(err)
}

func TestSupervisorLocalSafeHeadAdvancing(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	l2aChainID := out.L2CLA.ChainID()
	l2bChainID := out.L2CLB.ChainID()

	supervisorStatus := out.Supervisor.FetchSyncStatus()

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 2, "local-safe", LocalSafeHeadAdvanceRetries)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 2, "local-safe", LocalSafeHeadAdvanceRetries)

	err := wait.For(t.Ctx(), 5*time.Second, func() (bool, error) {
		latestSupervisorStatus := out.Supervisor.FetchSyncStatus()
		return latestSupervisorStatus.Chains[l2aChainID].LocalSafe.Number > supervisorStatus.Chains[l2aChainID].LocalSafe.Number &&
			latestSupervisorStatus.Chains[l2bChainID].LocalSafe.Number >= supervisorStatus.Chains[l2bChainID].LocalSafe.Number, nil
	})
	t.Require().NoError(err)
}

func TestSupervisorSafeHeadAdvancing(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	l2aChainID := out.L2CLA.ChainID()
	l2bChainID := out.L2CLB.ChainID()

	supervisorStatus := out.Supervisor.FetchSyncStatus()

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 2, "safe", SafeHeadAdvanceRetries)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 2, "safe", SafeHeadAdvanceRetries)

	err := wait.For(t.Ctx(), 5*time.Second, func() (bool, error) {
		latestSupervisorStatus := out.Supervisor.FetchSyncStatus()
		return latestSupervisorStatus.Chains[l2aChainID].CrossSafe.Number > supervisorStatus.Chains[l2aChainID].CrossSafe.Number &&
			latestSupervisorStatus.Chains[l2bChainID].CrossSafe.Number >= supervisorStatus.Chains[l2bChainID].CrossSafe.Number, nil
	})
	t.Require().NoError(err)
}

func TestSupervisorMinSyncedL1Advancing(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	supervisorStatus := out.Supervisor.FetchSyncStatus()

	out.Supervisor.AwaitMinL1(supervisorStatus.MinSyncedL1.Number + 1)

	err := wait.For(t.Ctx(), 5*time.Second, func() (bool, error) {
		latestSupervisorStatus := out.Supervisor.FetchSyncStatus()
		return latestSupervisorStatus.MinSyncedL1.Number > supervisorStatus.MinSyncedL1.Number, nil
	})
	t.Require().NoError(err)
}

func TestSupervisorFinalizedHeadAdvancing(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	l2aChainID := out.L2CLA.ChainID()
	l2bChainID := out.L2CLB.ChainID()

	supervisorStatus := out.Supervisor.FetchSyncStatus()

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 1, "finalized", FinalizedHeadAdvanceRetries)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 1, "finalized", FinalizedHeadAdvanceRetries)

	err := wait.For(t.Ctx(), 5*time.Second, func() (bool, error) {
		latestSupervisorStatus := out.Supervisor.FetchSyncStatus()
		return latestSupervisorStatus.Chains[l2aChainID].Finalized.Number > supervisorStatus.Chains[l2aChainID].Finalized.Number &&
			latestSupervisorStatus.Chains[l2bChainID].Finalized.Number >= supervisorStatus.Chains[l2bChainID].Finalized.Number, nil
	})
	t.Require().NoError(err)
}
