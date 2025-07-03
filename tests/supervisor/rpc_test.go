package supervisor

import (
	"context"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/shim"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/stack/match"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"testing"
)

func initSupervisorRPCClient(t devtest.T) stack.Supervisor {
	system := shim.NewSystem(t)
	orch := presets.Orchestrator()
	orch.Hydrate(system)
	return system.Supervisor(match.Assume(t, match.FirstSupervisor))
}

func TestRPCLocalUnsafe(gt *testing.T) {
	t := devtest.SerialT(gt)

	client := initSupervisorRPCClient(t)

	safe, err := client.QueryAPI().LocalUnsafe(context.Background(), eth.ChainIDFromUInt64(2151900)) // todo: avoid hardcoded chain id
	require.Error(t, err, "expected LocalUnsafe to failed for invalid chain")

	safe, err = client.QueryAPI().LocalUnsafe(context.Background(), eth.ChainIDFromUInt64(2151908))
	require.NoError(t, err, "expected LocalUnsafe to succeed for valid chain")
	assert.Greater(t, safe.Number, uint64(0), "block number should be greater than 0")
	assert.Len(t, safe.Hash, 32, "block hash should be 42 characters (0x-prefixed)")
}

func TestRPCCrossSafe(gt *testing.T) {
	t := devtest.SerialT(gt)

	client := initSupervisorRPCClient(t)

	safe, err := client.QueryAPI().CrossSafe(context.Background(), eth.ChainIDFromUInt64(2151900)) // todo: avoid hardcoded chain id
	require.Error(t, err, "expected CrossSafe to failed for invalid chain")

	safe, err = client.QueryAPI().CrossSafe(context.Background(), eth.ChainIDFromUInt64(2151908))
	require.NoError(t, err, "expected CrossSafe to succeed for valid chain")
	assert.Greater(t, safe.Derived, uint64(0), "block number should be greater than 0")
	assert.Len(t, safe.Derived, 32, "block hash should be 42 characters (0x-prefixed)")
}

func TestRPCFinalized(gt *testing.T) {
	t := devtest.SerialT(gt)

	client := initSupervisorRPCClient(t)

	safe, err := client.QueryAPI().Finalized(context.Background(), eth.ChainIDFromUInt64(2151900)) // todo: avoid hardcoded chain id
	require.Error(t, err, "expected Finalized to failed for invalid chain")

	safe, err = client.QueryAPI().Finalized(context.Background(), eth.ChainIDFromUInt64(2151908))
	require.NoError(t, err, "expected Finalized to succeed for valid chain")
	assert.Greater(t, safe, uint64(0), "block number should be greater than 0")
	assert.Len(t, safe, 32, "block hash should be 42 characters (0x-prefixed)")
}
