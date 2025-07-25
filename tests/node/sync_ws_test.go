package node

import (
	"sync"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/stretchr/testify/require"
)

// Check that unsafe heads eventually consolidate and become safe
// For this test to be deterministic it should...
// 1. Only have two L2 nodes
// 2. Only have one DA layer node
func TestSyncUnsafeBecomesSafe(gt *testing.T) {
	const SECS_WAIT_FOR_UNSAFE_HEAD = 10
	// We are waiting longer for the safe head to sync because it is usually a few seconds behind the unsafe head.
	const SECS_WAIT_FOR_SAFE_HEAD = 60

	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	var wg sync.WaitGroup
	for _, node := range nodes {
		wg.Add(1)
		go func(node *dsl.L2CLNode) {
			defer wg.Done()
			clName := node.Escape().ID().Key()
			clRPC, err := GetNodeRPCEndpoint(t.Ctx(), node)
			require.NoError(t, err, "failed to get RPC endpoint for node %s", clName)

			if !isKonaNode(t, clRPC, clName) {
				t.Log("node does not support ws endpoint, skipping sync test", clName)
				return
			}

			wsRPC := websocketRPC(clRPC)
			t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

			unsafeBlocks := GetKonaWS(t, wsRPC, "unsafe_head", time.After(SECS_WAIT_FOR_UNSAFE_HEAD*time.Second))

			safeBlocks := GetKonaWS(t, wsRPC, "safe_head", time.After(SECS_WAIT_FOR_SAFE_HEAD*time.Second))

			require.GreaterOrEqual(t, len(unsafeBlocks), 1, "we didn't receive enough unsafe gossip blocks!")
			require.GreaterOrEqual(t, len(safeBlocks), 1, "we didn't receive enough safe gossip blocks!")

			safeBlockMap := make(map[uint64]eth.L2BlockRef)
			// Create a map of safe blocks with block number as the key
			for _, safeBlock := range safeBlocks {
				safeBlockMap[safeBlock.Number] = safeBlock
			}

			cond := false

			// Iterate over unsafe blocks and find matching safe blocks
			for _, unsafeBlock := range unsafeBlocks {
				if safeBlock, exists := safeBlockMap[unsafeBlock.Number]; exists {
					require.Equal(t, unsafeBlock, safeBlock, "unsafe block %d doesn't match safe block %d", unsafeBlock.Number, safeBlock.Number)
					cond = true
				}
			}

			require.True(t, cond, "No matching safe block found for unsafe block")

			t.Log("✓ unsafe and safe head blocks match between all nodes")
		}(&node)
	}
	wg.Wait()
}

// System tests that ensure that the kona-nodes are syncing the unsafe chain.
// Note: this test should only be ran on networks that don't reorg, only have one sequencer and that only have one DA layer node.
func TestSyncUnsafe(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	var wg sync.WaitGroup
	for _, node := range nodes {
		wg.Add(1)
		go func(node *dsl.L2CLNode) {
			defer wg.Done()
			clName := node.Escape().ID().Key()
			clRPC, err := GetNodeRPCEndpoint(t.Ctx(), node)
			require.NoError(t, err, "failed to get RPC endpoint for node %s", clName)

			if !isKonaNode(t, clRPC, clName) {
				t.Log("node does not support ws endpoint, skipping sync test", clName)
				return
			}

			wsRPC := websocketRPC(clRPC)
			t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

			output := GetKonaWS(t, wsRPC, "unsafe_head", time.After(10*time.Second))

			// For each block, we check that the block is actually in the chain of the other nodes.
			// That should always be the case unless there is a reorg or a long sync.
			// We shouldn't have safe heads reorgs in this very simple testnet because there is only one DA layer node.
			for _, block := range output {
				for _, node := range nodes {
					otherCLRPC, err := GetNodeRPCEndpoint(t.Ctx(), &node)
					require.NoError(t, err, "failed to get RPC endpoint for node %s", node.Escape().ID().Key())
					otherCLNode := node.Escape().ID().Key()

					syncStatus := &eth.SyncStatus{}
					require.NoError(t, SendRPCRequest(otherCLRPC, "optimism_syncStatus", syncStatus), "impossible to get sync status from node %s", otherCLNode)

					if syncStatus.UnsafeL2.Number < block.Number {
						t.Log("✗ peer too far behind!", otherCLNode, block.Number, syncStatus.UnsafeL2.Number)
						continue
					}

					expectedOutputResponse := eth.OutputResponse{}
					require.NoError(t, SendRPCRequest(otherCLRPC, "optimism_outputAtBlock", &expectedOutputResponse, hexutil.Uint64(block.Number)), "impossible to get block from node %s", otherCLNode)

					// Make sure the blocks match!
					require.Equal(t, expectedOutputResponse.BlockRef, block, "block mismatch between %s and %s", otherCLNode, clName)
				}
			}

			t.Log("✓ unsafe head blocks match between all nodes")
		}(&node)
	}
	wg.Wait()
}

// System tests that ensure that the kona-nodes are syncing the safe chain.
// Note: this test should only be ran on networks that don't reorg and that only have one DA layer node.
func TestSyncSafe(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	var wg sync.WaitGroup
	for _, node := range nodes {
		wg.Add(1)
		go func(node *dsl.L2CLNode) {
			defer wg.Done()
			clName := node.Escape().ID().Key()
			clRPC, err := GetNodeRPCEndpoint(t.Ctx(), node)
			require.NoError(t, err, "failed to get RPC endpoint for node %s", clName)

			if !isKonaNode(t, clRPC, clName) {
				t.Log("node does not support ws endpoint, skipping sync test", clName)
				return
			}

			wsRPC := websocketRPC(clRPC)
			t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

			output := GetKonaWS(t, wsRPC, "safe_head", time.After(10*time.Second))

			// For each block, we check that the block is actually in the chain of the other nodes.
			// That should always be the case unless there is a reorg or a long sync.
			// We shouldn't have safe heads reorgs in this very simple testnet because there is only one DA layer node.
			for _, block := range output {
				for _, node := range nodes {
					otherCLRPC, err := GetNodeRPCEndpoint(t.Ctx(), &node)
					require.NoError(t, err, "failed to get RPC endpoint for node %s", node.Escape().ID().Key())
					otherCLNode := node.Escape().ID().Key()

					syncStatus := &eth.SyncStatus{}
					require.NoError(t, SendRPCRequest(otherCLRPC, "optimism_syncStatus", syncStatus), "impossible to get sync status from node %s", otherCLNode)

					if syncStatus.SafeL2.Number < block.Number {
						t.Log("✗ peer too far behind!", otherCLNode, block.Number, syncStatus.SafeL2.Number)
						continue
					}

					expectedOutputResponse := eth.OutputResponse{}
					require.NoError(t, SendRPCRequest(otherCLRPC, "optimism_outputAtBlock", &expectedOutputResponse, hexutil.Uint64(block.Number)), "impossible to get block from node %s", otherCLNode)

					// Make sure the blocks match!
					require.Equal(t, expectedOutputResponse.BlockRef, block, "block mismatch between %s and %s", otherCLNode, clName)
				}
			}

			t.Log("✓ safe head blocks match between all nodes")
		}(&node)
	}
	wg.Wait()
}

// System tests that ensure that the kona-nodes are syncing the finalized chain.
// Note: this test can be ran on any sort of network, including the ones that should reorg.
func TestSyncFinalized(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	nodes := out.L2CLNodes()

	var wg sync.WaitGroup
	for _, node := range nodes {
		wg.Add(1)
		go func(node *dsl.L2CLNode) {
			defer wg.Done()
			clName := node.Escape().ID().Key()
			clRPC, err := GetNodeRPCEndpoint(t.Ctx(), node)
			require.NoError(t, err, "failed to get RPC endpoint for node %s", clName)

			if !isKonaNode(t, clRPC, clName) {
				t.Log("node does not support ws endpoint, skipping sync test", clName)
				return
			}

			wsRPC := websocketRPC(clRPC)
			t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

			output := GetKonaWS(t, wsRPC, "finalized_head", time.After(4*time.Minute))

			// We should check that we received at least 2 finalized blocks within 4 minutes!
			require.Greater(t, len(output), 1, "we didn't receive enough finalized gossip blocks!")
			t.Log("Number of finalized blocks received within 4 minutes:", len(output))

			// For each block, we check that the block is actually in the chain of the other nodes.
			for _, block := range output {
				for _, node := range nodes {
					otherCLRPC, err := GetNodeRPCEndpoint(t.Ctx(), &node)
					require.NoError(t, err, "failed to get RPC endpoint for node %s", node.Escape().ID().Key())
					otherCLNode := node.Escape().ID().Key()

					syncStatus := &eth.SyncStatus{}
					require.NoError(t, SendRPCRequest(otherCLRPC, "optimism_syncStatus", syncStatus), "impossible to get sync status from node %s", otherCLNode)

					if syncStatus.FinalizedL2.Number < block.Number {
						t.Log("✗ peer too far behind!", otherCLNode, block.Number, syncStatus.FinalizedL2.Number)
						continue
					}

					expectedOutputResponse := eth.OutputResponse{}
					require.NoError(t, SendRPCRequest(otherCLRPC, "optimism_outputAtBlock", &expectedOutputResponse, hexutil.Uint64(block.Number)), "impossible to get block from node %s", otherCLNode)

					// Make sure the blocks match!
					require.Equal(t, expectedOutputResponse.BlockRef, block, "block mismatch between %s and %s", otherCLNode, clName)
				}
			}

			t.Log("✓ finalized head blocks match between all nodes")
		}(&node)
	}
	wg.Wait()
}
