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
