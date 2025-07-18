package preinterop

import (
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-node/rollup"
	stypes "github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
)

func TestPostInteropStatusSync(gt *testing.T) {
	t := devtest.ParallelT(gt)
	sys := presets.NewSimpleInterop(t)
	require := t.Require()

	devtest.RunParallel(t, sys.L2Networks(), func(t devtest.T, net *dsl.L2Network) {
		t.Logger().Info("Awaiting activation block for chain", "chainID", net.ChainID())
		activationBlock := net.AwaitActivation(t, rollup.Interop)
		require.NotNil(activationBlock, "ActivationBlock should return a valid block number")

		t.Logger().Info("Activation block found", "blockNumber", activationBlock.Number)
		time.Sleep(10 * time.Second)

		// wait for some time to ensure the interop activation block is become cross-safe
		t.Logger().Info("Waiting for interop activation block to be cross-safe")
		sys.Supervisor.WaitForL2HeadToAdvanceTo(net.ChainID(), stypes.CrossSafe, activationBlock)

		status, err := sys.Supervisor.Escape().QueryAPI().SyncStatus(t.Ctx())
		require.NoError(err, "SyncStatus should not error after interop")
		require.NotNil(status, "SyncStatus should return a valid status")
	})
}

func TestSupervisorActivationBlock(gt *testing.T) {
	t := devtest.ParallelT(gt)
	sys := presets.NewSimpleInterop(t)
	require := t.Require()

	devtest.RunParallel(t, sys.L2Networks(), func(t devtest.T, net *dsl.L2Network) {
		t.Logger().Info("Awaiting activation block for chain", "chainID", net.ChainID())

		activationBlock := net.AwaitActivation(t, rollup.Interop)
		require.NotNil(activationBlock, "ActivationBlock should return a valid block number")

		t.Logger().Info("Activation block found", "blockNumber", activationBlock.Number)
		time.Sleep(10 * time.Second)

		// wait for some time to ensure the interop activation block is become cross-safe
		t.Logger().Info("Waiting for interop activation block to be cross-safe")
		sys.Supervisor.WaitForL2HeadToAdvanceTo(net.ChainID(), stypes.CrossSafe, activationBlock)

		interopTime := net.Escape().ChainConfig().InteropTime
		pre := net.LatestBlockBeforeTimestamp(t, *interopTime)
		require.NotNil(pre, "Pre-interop block should be found before interop time")

		// make sure pre-interop block is parent of activation block
		require.Equal(pre.Number, activationBlock.Number-1, "Activation block should be one after pre-interop block")

		// fetching the source for the pre-interop block should return the error
		// this is to make sure that we only store the blocks after interop
		_, err := sys.Supervisor.Escape().QueryAPI().CrossDerivedToSource(t.Ctx(), net.ChainID(), pre.ID())
		require.Error(err, "CrossDerivedToSource should error before interop")

		// fetch the source for the activation block
		derivedFrom, err := sys.Supervisor.Escape().QueryAPI().CrossDerivedToSource(t.Ctx(), net.ChainID(), activationBlock)
		require.NoError(err, "CrossDerivedToSource should not error after interop")
		require.NotNil(derivedFrom, "CrossDerivedToSource should return a valid source block")
	})
}
