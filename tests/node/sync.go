package node

import (
	"strings"
	"time"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/op-service/apis"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/stretchr/testify/require"
)

// push { "jsonrpc":"2.0", "method":"time", "params":{ "subscription":"0x…", "result":"…" } }
type push struct {
	Method string `json:"method"`
	Params struct {
		SubID  uint64         `json:"subscription"`
		Result eth.L2BlockRef `json:"result"`
	} `json:"params"`
}

func websocketRPC(clRPC string) string {
	// Remove the leading http and replace it with ws.
	return strings.Replace(clRPC, "http", "ws", 1)
}

func nodeSupportsKonaWs(t systest.T, clRPC string, clName string) bool {
	peerInfo := &apis.PeerInfo{}

	require.NoError(t, SendRPCRequest(clRPC, "opp2p_self", peerInfo), "failed to send RPC request to node %s: %s", clName)

	// For now, the ws endpoint is only supported by kona nodes.
	if !strings.Contains(strings.ToLower(peerInfo.UserAgent), "kona") {
		return false
	}

	return true
}

// System tests that ensure that the kona-nodes are syncing the safe chain.
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
