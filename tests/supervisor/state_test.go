package supervisor

import (
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-e2e/e2eutils/wait"
)

func TestSupervisorLocalUnsafeHeadAdvancing(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := presets.NewSimpleInterop(t)
	l2aChainID := out.L2CLA.ChainID()
	l2bChainID := out.L2CLB.ChainID()

	supervisorStatus := out.Supervisor.FetchSyncStatus()

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 2, "unsafe", 5)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 2, "unsafe", 5)

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

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 2, "cross-unsafe", 5)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 2, "cross-unsafe", 5)

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

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 2, "local-safe", 15)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 2, "local-safe", 15)

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

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 2, "safe", 25)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 2, "safe", 25)

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

	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainA.ChainID(), 1, "finalized", 100)
	out.Supervisor.WaitForL2HeadToAdvance(out.L2ChainB.ChainID(), 1, "finalized", 100)

	err := wait.For(t.Ctx(), 5*time.Second, func() (bool, error) {
		latestSupervisorStatus := out.Supervisor.FetchSyncStatus()
		return latestSupervisorStatus.Chains[l2aChainID].Finalized.Number > supervisorStatus.Chains[l2aChainID].Finalized.Number &&
			latestSupervisorStatus.Chains[l2bChainID].Finalized.Number >= supervisorStatus.Chains[l2bChainID].Finalized.Number, nil
	})
	t.Require().NoError(err)
}
