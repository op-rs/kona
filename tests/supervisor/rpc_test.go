package supervisor

import (
	"context"
	"fmt"
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/shim"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/stack/match"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func setupTestClient(t devtest.T) (stack.Supervisor, []eth.ChainID) {
	system := shim.NewSystem(t)
	orch := presets.Orchestrator()
	orch.Hydrate(system)

	ids := system.L2NetworkIDs()
	require.GreaterOrEqual(t, len(ids), 2, "at least 2 L2 networks required")

	var chainIDs []eth.ChainID
	for _, id := range ids {
		chainIDs = append(chainIDs, id.ChainID())
	}

	client := system.Supervisor(match.Assume(t, match.FirstSupervisor))
	return client, chainIDs
}

func TestRPCLocalUnsafe(gt *testing.T) {
	t := devtest.ParallelT(gt)

	client, chainIDs := setupTestClient(t)

	t.Run("fails with invalid chain ID", func(gt devtest.T) {
		_, err := client.QueryAPI().LocalUnsafe(context.Background(), eth.ChainIDFromUInt64(100))
		require.Error(t, err, "expected LocalUnsafe to fail with raw chain ID")
	})

	for _, chainID := range chainIDs {
		t.Run(fmt.Sprintf("succeeds with valid chain ID %d", chainID), func(gt devtest.T) {
			safe, err := client.QueryAPI().LocalUnsafe(context.Background(), chainID)
			require.NoError(t, err)
			assert.Greater(t, safe.Number, uint64(0))
			assert.Len(t, safe.Hash, 32)
		})
	}
}

func TestRPCCrossSafe(gt *testing.T) {
	t := devtest.ParallelT(gt)

	client, chainIDs := setupTestClient(t)

	t.Run("fails with invalid chain ID", func(gt devtest.T) {
		_, err := client.QueryAPI().CrossSafe(context.Background(), eth.ChainIDFromUInt64(100))
		require.Error(t, err, "expected CrossSafe to fail with invalid chain")
	})

	for _, chainID := range chainIDs {
		t.Run(fmt.Sprintf("succeeds with valid chain ID %d", chainID), func(gt devtest.T) {
			blockPair, err := client.QueryAPI().CrossSafe(context.Background(), chainID)
			require.NoError(t, err)
			assert.Greater(t, blockPair.Derived.Number, uint64(0))
			assert.Len(t, blockPair.Derived.Hash, 32)

			assert.Greater(t, blockPair.Source.Number, uint64(0))
			assert.Len(t, blockPair.Source.Hash, 32)
		})
	}
}

func TestRPCFinalized(gt *testing.T) {
	t := devtest.ParallelT(gt)

	client, chainIDs := setupTestClient(t)

	t.Run("fails with invalid chain ID", func(gt devtest.T) {
		_, err := client.QueryAPI().Finalized(context.Background(), eth.ChainIDFromUInt64(100))
		require.Error(t, err, "expected Finalized to fail with invalid chain")
	})

	for _, chainID := range chainIDs {
		t.Run(fmt.Sprintf("succeeds with valid chain ID %d", chainID), func(gt devtest.T) {
			safe, err := client.QueryAPI().Finalized(context.Background(), chainID)
			require.NoError(t, err)
			assert.Greater(t, safe.Number, uint64(0))
			assert.Len(t, safe.Hash, 32)
		})
	}
}
