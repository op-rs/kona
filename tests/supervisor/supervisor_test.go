package supervisor

import (
	"testing"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/stretchr/testify/require"
)

func TestInteropSystemSupervisor(t *testing.T) {
	systest.InteropSystemTest(t, func(t systest.T, sys system.InteropSystem) {
		ctx := t.Context()

		supervisor, err := sys.Supervisor(ctx)
		require.NoError(t, err)
		_, err = supervisor.SyncStatus(ctx)
		require.NoError(t, err)
	})
}

// func TestInitExecMsg(gt *testing.T) {
// 	t := devtest.SerialT(gt)
// 	sys := presets.NewSimpleInterop(t)

// 	t.Require().NotEqual(sys.L2ChainA.ChainID(), sys.L2ChainB.ChainID(), "sanity-check we have two different chains")
// 	t.Logf("L2 Chain A: %s, L2 Chain B: %s", sys.L2ChainA.ChainID(), sys.L2ChainB.ChainID())

// 	// rng := rand.New(rand.NewSource(1234))
// 	// alice := sys.FunderA.NewFundedEOA(eth.OneEther)
// 	// bob := sys.FunderB.NewFundedEOA(eth.OneEther)

// 	// t.Logf("alice: %s, bob: %s", alice.Address(), bob.Address())

// 	// eventLoggerAddress := alice.DeployEventLogger()
// 	// Trigger random init message at chain A
// 	// _, _ = alice.SendInitMessage(interop.RandomInitTrigger(rng, eventLoggerAddress, rng.Intn(5), rng.Intn(30)))
// 	// Make sure supervisor indexes block which includes init message
// 	// sys.Supervisor.AdvancedUnsafeHead(alice.ChainID(), 2)
// 	// // Single event in tx so index is 0
// 	// bob.SendExecMessage(initIntent, 0)
// }
