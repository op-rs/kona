package node

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/testlib/validators"
	"github.com/stretchr/testify/require"
)

type rpcRequest struct {
	JSONRPC string      `json:"jsonrpc"`
	Method  string      `json:"method"`
	Params  interface{} `json:"params"`
	ID      uint64      `json:"id"`
}

type rpcResponse struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      uint64          `json:"id"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   *rpcError       `json:"error,omitempty"`
}

type rpcError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

// Verify that all the EL nodes are able to find peers.
func peerCount() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()

		for _, l2 := range l2s {

			nodes := l2.Nodes()

			for _, node := range nodes {
				client, err := node.GethClient()

				require.NoError(t, err, "Failed to get geth client for node %s", node.Name())

				pc, err := client.PeerCount(t.Context())

				require.NoError(t, err, "Failed to get peer count for node %s", node.Name())

				require.Greater(t, pc, uint64(0), "Peer count for node %s is 0", node.Name())

				clName := node.CLName()
				clRPC := node.CLRPC()

				// Curl the CL RPC endpoint `opp2p_self` using RPC
				// 1. Build the payload.
				reqBody := rpcRequest{
					JSONRPC: "2.0",
					Method:  "opp2p_self",
					Params:  []interface{}{},
					ID:      1,
				}
				payload, err := json.Marshal(reqBody)
				if err != nil {
					panic(err)
				}

				// 2. Configure an HTTP client with sensible timeouts.
				httpClient := &http.Client{
					Timeout: 10 * time.Second,
				}

				// 3. Create a context (optional, lets you cancel).
				ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
				defer cancel()

				// 4. Build the HTTP request.
				req, err := http.NewRequestWithContext(ctx, http.MethodPost,
					clRPC, bytes.NewReader(payload))
				if err != nil {
					panic(err)
				}
				req.Header.Set("Content-Type", "application/json")

				// 5. Send the request.
				resp, err := httpClient.Do(req)
				if err != nil {
					panic(err)
				}
				defer resp.Body.Close()

				// 6. Read and decode the response.
				respBytes, err := io.ReadAll(resp.Body)
				if err != nil {
					panic(err)
				}

				var rpcResp rpcResponse
				if err := json.Unmarshal(respBytes, &rpcResp); err != nil {
					panic(err)
				}

				// 7. Check for RPC‑level errors.
				if rpcResp.Error != nil {
					panic(fmt.Errorf("rpc error %d: %s", rpcResp.Error.Code, rpcResp.Error.Message))
				}

				// 8. Use the result (here we just print it).
				t.Log("Raw result: %s\n", string(rpcResp.Result))

				t.Log("CL Name:", clName, "CL RPC:", clRPC)

			}
		}
	}
}

func TestPeerCount(t *testing.T) {
	// Get the L2 chain we want to test with
	chainIdx := uint64(0) // First L2 chain

	nodeValidator := validators.HasSufficientL2Nodes(chainIdx, 1)

	systest.SystemTest(t,
		peerCount(),
		nodeValidator,
	)
}

func TestSyncStart(t *testing.T) {
	// Get the L2 chain we want to test with
	chainIdx := uint64(0) // First L2 chain

	nodeValidator := validators.HasSufficientL2Nodes(chainIdx, 1)

	systest.SystemTest(t,
		peerCount(),
		nodeValidator,
	)
}
