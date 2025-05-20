package node

import (
	"encoding/json"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
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

				rpcResp, err := SendRPCRequest(clRPC, "opp2p_peerStats", nil)

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

				if peerStats.Known == 0 {
					t.Errorf("node %s has no known peers", clName)
				}

				if peerStats.Table == 0 {
					t.Errorf("node %s has no peers in the discovery table", clName)
				}

				if peerStats.Connected == 0 {
					t.Errorf("node %s has no peers", clName)
				}

				if peerStats.BlocksTopicV4 == 0 {
					t.Errorf("node %s has no blocks topic v4 peers", clName)
				}

				if peerStats.BlocksTopicV3 == 0 {
					t.Errorf("node %s has no blocks topic v3 peers", clName)
				}

				if peerStats.BlocksTopicV2 == 0 {
					t.Errorf("node %s has no blocks topic v2 peers", clName)
				}

				if peerStats.BlocksTopic == 0 {
					t.Errorf("node %s has no blocks topic peers", clName)
				}
			}
		}
	}
}
