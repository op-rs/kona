package node

import (
	"encoding/json"
	"testing"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/testlib/validators"
	"github.com/ethereum-optimism/optimism/op-service/apis"
)

// Really simple system test that just checks that each node has at least 1 peer that is connected to its topics.
func peerCount() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()

		for _, l2 := range l2s {
			for _, node := range l2.Nodes() {
				clRPC := node.CLRPC()
				clName := node.CLName()

				// 1. Build the payload.
				reqBody := rpcRequest{
					JSONRPC: "2.0",
					Method:  "opp2p_peerStats", // example: Ethereum JSON‑RPC
					Params:  []any{},
					ID:      1,
				}

				rpcResp, err := reqBody.SendRPCRequest(clRPC)

				if err != nil {
					t.Errorf("failed to send RPC request to node %s: %s", clName, err)
				} else if rpcResp.Error != nil {
					t.Errorf("received RPC error from node %s: %s", clName, rpcResp.Error)
				}

				peerStats := apis.PeerStats{}

				err = json.Unmarshal(rpcResp.Result, &peerStats)

				if err != nil {
					t.Errorf("failed to unmarshal result: %s", err)
				}

				if peerStats.Connected < 1 {
					t.Errorf("node %s has no peers", clName)
				}
			}
		}
	}
}

func TestSystemKonaTest(t *testing.T) {
	// Get the L2 chain we want to test with
	chainIdx := uint64(0) // First L2 chain

	nodeValidator := validators.HasSufficientL2Nodes(chainIdx, 2)

	systest.SystemTest(t,
		peerCount(),
		nodeValidator,
	)
}
