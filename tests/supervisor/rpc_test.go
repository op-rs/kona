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
	"github.com/ethereum/go-ethereum/common"
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

func TestRPCAllSafeDerivedAt(gt *testing.T) {
	t := devtest.ParallelT(gt)

	client, chainIDs := setupTestClient(t)

	t.Run("fails with invalid L1 block hash", func(gt devtest.T) {
		_, err := client.QueryAPI().AllSafeDerivedAt(context.Background(), eth.BlockID{
			Number: 100,
			Hash:   common.HexToHash("0x0000000000000000000000000000000000000000000000000000000000000000"),
		})
		require.Error(t, err)
	})

	t.Run("succeeds with valid synced L1 block hash", func(gt devtest.T) {
		sync, err := client.QueryAPI().SyncStatus(context.Background())
		require.NoError(t, err)

		allSafe, err := client.QueryAPI().AllSafeDerivedAt(context.Background(), eth.BlockID{
			Number: sync.MinSyncedL1.Number,
			Hash:   sync.MinSyncedL1.Hash,
		})
		require.NoError(t, err)

		require.Equal(t, len(chainIDs), len(allSafe))
		for key, value := range allSafe {
			require.Contains(t, chainIDs, key)
			require.Len(t, value.Hash, 32)
		}
	})
}

func TestRPCCrossDerivedToSource(gt *testing.T) {
	t := devtest.ParallelT(gt)

	client, chainIDs := setupTestClient(t)

	t.Run("fails with invalid chain ID", func(gt devtest.T) {
		_, err := client.QueryAPI().CrossDerivedToSource(context.Background(), eth.ChainIDFromUInt64(100), eth.BlockID{Number: 25})
		require.Error(t, err, "expected CrossDerivedToSource to fail with invalid chain")
	})

	safe, err := client.QueryAPI().CrossSafe(context.Background(), chainIDs[0])
	require.NoError(t, err)

	t.Run(fmt.Sprintf("succeeds with valid chain ID %d", chainIDs[0]), func(gt devtest.T) {
		source, err := client.QueryAPI().CrossDerivedToSource(
			context.Background(),
			chainIDs[0],
			eth.BlockID{
				Number: safe.Derived.Number,
				Hash:   safe.Derived.Hash,
			},
		)
		require.NoError(t, err)
		assert.Greater(t, source.Number, uint64(0))
		assert.Len(t, source.Hash, 32)
		assert.Equal(t, source.Number, safe.Source.Number)
		assert.Equal(t, source.Hash, safe.Source.Hash)
	})

}
