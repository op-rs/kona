package node

import (
	"time"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/stretchr/testify/require"
)

// System tests that ensure that the kona-nodes are syncing the unsafe chain.
func syncUnsafe() systest.SystemTestFunc {
	const SECS_WAIT_FOR_UNSAFE_HEAD = 10

	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		for _, l2 := range l2s {
			for _, node := range l2.Nodes() {
				clRPC := node.CLRPC()
				clName := node.CLName()

				if !nodeSupportsKonaWs(t, clRPC, clName) {
					t.Log("node does not support ws endpoint, skipping sync test", clName)
					continue
				}

				wsRPC := websocketRPC(clRPC)
				t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

				output := GetKonaWS(t, wsRPC, "unsafe_head", time.After(SECS_WAIT_FOR_UNSAFE_HEAD*time.Second))

				// We can't really check whether the unsafe gossip match between the nodes of the network because
				// the unsafe head can't be trusted.
				// We'll just ensure that we have at least received a decent number of unsafe gossip (1 per 2 seconds).
				require.GreaterOrEqual(t, len(output), SECS_WAIT_FOR_UNSAFE_HEAD/2, "we didn't receive enough unsafe gossip blocks!")
			}
		}
	}
}

// System tests that ensure that the kona-nodes are syncing the unsafe chain.
// This test is meant to be ran on networks that only have two nodes. This will ensure that the gossiped unsafe head
// is correctly recevied by the other node.
func syncUnsafeTwoNodes() systest.SystemTestFunc {
	const SECS_WAIT_FOR_UNSAFE_HEAD = 10

	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		for _, l2 := range l2s {
			require.LessOrEqual(t, len(l2.Nodes()), 2, "we can only test unsafe sync deterministically on networks with at most 2 nodes")
			seqNode := l2.Nodes()[0]
			konaNode := l2.Nodes()[1]

			clRPC := konaNode.CLRPC()
			clName := konaNode.CLName()

			require.True(t, nodeSupportsKonaWs(t, clRPC, clName), "kona node doesn't support ws endpoint")

			wsRPC := websocketRPC(clRPC)
			t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

			output := GetKonaWS(t, wsRPC, "unsafe_head", time.After(SECS_WAIT_FOR_UNSAFE_HEAD*time.Second))

			// For each block, we check that the block is actually in the chain of the sequencer node.
			// That should always be the case since we only have two nodes.
			for _, block := range output {
				seqCLRPC := seqNode.CLRPC()
				seqCLNode := seqNode.CLName()

				syncStatus := &eth.SyncStatus{}
				require.NoError(t, SendRPCRequest(seqCLRPC, "optimism_syncStatus", syncStatus), "impossible to get sync status from node %s", seqCLNode)

				if syncStatus.UnsafeL2.Number < block.Number {
					t.Log("✗ peer too far behind!", seqCLNode, block.Number, syncStatus.SafeL2.Number)
					continue
				}

				expectedOutputResponse := eth.OutputResponse{}
				require.NoError(t, SendRPCRequest(seqCLRPC, "optimism_outputAtBlock", &expectedOutputResponse, hexutil.Uint64(block.Number)), "impossible to get block from node %s", seqCLNode)

				// Make sure the blocks match!
				require.Equal(t, expectedOutputResponse.BlockRef, block, "block mismatch between %s and %s", seqCLNode, clName)
			}

			t.Log("✓ unsafe head blocks match between all nodes")
		}
	}
}

// Check that unsafe heads eventually consolidate and become safe
// For this test to be deterministic it should...
// 1. Only have two L2 nodes
// 2. Only have one DA layer node
func syncUnsafeBecomesSafe() systest.SystemTestFunc {
	const SECS_WAIT_FOR_UNSAFE_HEAD = 10
	const SECS_WAIT_FOR_SAFE_HEAD = 30

	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		for _, l2 := range l2s {
			require.LessOrEqual(t, len(l2.Nodes()), 2, "we can only test unsafe sync deterministically on networks with at most 2 nodes")
			konaNode := l2.Nodes()[1]

			clRPC := konaNode.CLRPC()
			clName := konaNode.CLName()

			require.True(t, nodeSupportsKonaWs(t, clRPC, clName), "kona node doesn't support ws endpoint")

			wsRPC := websocketRPC(clRPC)
			t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

			unsafeBlocks := GetKonaWS(t, wsRPC, "unsafe_head", time.After(SECS_WAIT_FOR_UNSAFE_HEAD*time.Second))

			safeBlocks := GetKonaWS(t, wsRPC, "safe_head", time.After(SECS_WAIT_FOR_SAFE_HEAD*time.Second))

			// Wait for the Goroutine to complete

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
		}
	}
}

// System tests that ensure that the kona-nodes are syncing the safe chain.
// Note: this test should only be ran on networks that don't reorg and that only have one DA layer node.
func syncSafe() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		for _, l2 := range l2s {
			for _, node := range l2.Nodes() {
				clRPC := node.CLRPC()
				clName := node.CLName()

				if !nodeSupportsKonaWs(t, clRPC, clName) {
					t.Log("node does not support ws endpoint, skipping sync test", clName)
					continue
				}

				wsRPC := websocketRPC(clRPC)
				t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

				output := GetKonaWS(t, wsRPC, "safe_head", time.After(10*time.Second))

				// For each block, we check that the block is actually in the chain of the other nodes.
				// That should always be the case unless there is a reorg or a long sync.
				// We shouldn't have safe heads reorgs in this very simple testnet because there is only one DA layer node.
				for _, block := range output {
					for _, node := range l2.Nodes() {
						otherCLRPC := node.CLRPC()
						otherCLNode := node.CLName()

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
			}
		}
	}
}

// System tests that ensure that the kona-nodes are syncing the finalized chain.
// Note: this test can be ran on any sort of network, including the ones that should reorg.
func syncFinalized() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()
		for _, l2 := range l2s {
			for _, node := range l2.Nodes() {
				clRPC := node.CLRPC()
				clName := node.CLName()

				if !nodeSupportsKonaWs(t, clRPC, clName) {
					t.Log("node does not support ws endpoint, skipping sync test", clName)
					continue
				}

				wsRPC := websocketRPC(clRPC)
				t.Log("node supports ws endpoint, continuing sync test", clName, wsRPC)

				output := GetKonaWS(t, wsRPC, "finalized_head", time.After(10*time.Second))

				// For each block, we check that the block is actually in the chain of the other nodes.
				for _, block := range output {
					for _, node := range l2.Nodes() {
						otherCLRPC := node.CLRPC()
						otherCLNode := node.CLName()

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
			}
		}
	}
}
