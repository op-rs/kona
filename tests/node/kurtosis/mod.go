package node_kurtosis

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/gorilla/websocket"
	"github.com/kurtosis-tech/kurtosis/api/golang/engine/lib/kurtosis_context"
	"github.com/stretchr/testify/require"
)

// --- Generic RPC request/response types -------------------------------------
const (
	DEFAULT_TIMEOUT = 10 * time.Second
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

// push { "jsonrpc":"2.0", "method":"time", "params":{ "subscription":"0x…", "result":"…" } }
type push[Out any] struct {
	Method string `json:"method"`
	Params struct {
		SubID  uint64 `json:"subscription"`
		Result Out    `json:"result"`
	} `json:"params"`
}

func WebsocketRPC(clRPC string) string {
	// Remove the leading http and replace it with ws.
	return strings.Replace(clRPC, "http", "ws", 1)
}

func supportsDevRPC(t devtest.T, clName string, clRPC string) bool {
	// To see if the node supports the dev RPC, we try to send a request to the dev RPC to
	// get the last engine queue length.
	engineQueueLength := 0
	if err := SendRPCRequest(clRPC, "dev_taskQueueLength", &engineQueueLength); err != nil {
		return false
	}

	return true
}

// ---------------------------------------------------------------------------

// GetCPUStats executes shell commands to get CPU usage statistics from a service
func RpcEndpoint(ctx context.Context, serviceName string) (string, error) {
	kurtosisCtx, err := kurtosis_context.NewKurtosisContextFromLocalEngine()
	if err != nil {
		return "", err
	}

	enclaves, err := kurtosisCtx.GetEnclaves(ctx)
	if err != nil {
		return "", err
	}

	for enclave := range enclaves.GetEnclavesByName() {
		enclaveCtx, err := kurtosisCtx.GetEnclaveContext(ctx, enclave)
		if err != nil {
			return "", err
		}

		serviceCtx, err := enclaveCtx.GetServiceContext(serviceName)
		if err != nil {
			return "", err
		}

		publicPorts := serviceCtx.GetPublicPorts()

		// Get the port for the RPC endpoint
		rpcPort, ok := publicPorts["rpc"]
		if !ok {
			return "", fmt.Errorf("rpc port not found")
		}

		// Get the RPC endpoint
		applicationProtocol := rpcPort.GetMaybeApplicationProtocol()
		if applicationProtocol == "" {
			applicationProtocol = "http"
		}

		publicIPAddress := serviceCtx.GetMaybePublicIPAddress()
		if publicIPAddress == "" {
			return "", fmt.Errorf("public IP address not found")
		}

		return fmt.Sprintf("%s://%s:%d", applicationProtocol, publicIPAddress, rpcPort.GetNumber()), nil
	}

	return "", fmt.Errorf("no enclaves found")
}

func GetNodeRPCEndpoint(ctx context.Context, node *dsl.L2CLNode) (string, error) {
	return RpcEndpoint(ctx, node.Escape().ID().Key())
}

func SendRPCRequest[T any](addr string, method string, resOutput *T, params ...any) error {
	// 1. Build the payload.
	s := rpcRequest{
		JSONRPC: "2.0",
		Method:  method,
		Params:  params,
		ID:      1,
	}

	// 1. Marshal the request.
	payload, err := json.Marshal(s)
	if err != nil {
		return err
	}

	// 2. Configure an HTTP client with sensible timeouts.
	client := &http.Client{
		Timeout: DEFAULT_TIMEOUT,
	}

	// 3. Create a context (optional, lets you cancel).
	ctx, cancel := context.WithTimeout(context.Background(), DEFAULT_TIMEOUT)
	defer cancel()

	// 4. Build the HTTP request.
	req, err := http.NewRequestWithContext(ctx, http.MethodPost,
		addr, bytes.NewReader(payload))

	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/json")

	// 5. Send the request.
	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	// 6. Read and decode the response.
	respBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return err
	}

	var rpcResp rpcResponse
	if err := json.Unmarshal(respBytes, &rpcResp); err != nil {
		return err
	}

	if resOutput != nil {
		err = json.Unmarshal(rpcResp.Result, resOutput)

		if err != nil {
			return err
		}
	}

	return nil

}

func GetPrefixedWs[T any, Out any](t devtest.T, prefix string, wsRPC string, method string, runUntil <-chan T) []Out {
	conn, _, err := websocket.DefaultDialer.DialContext(t.Ctx(), wsRPC, nil)
	require.NoError(t, err, "dial: %v", err)
	defer conn.Close()

	// 1. send the *_subscribe request
	require.NoError(t, conn.WriteJSON(rpcRequest{
		JSONRPC: "2.0",
		ID:      1,
		Method:  prefix + "_" + "subscribe_" + method,
		Params:  nil,
	}), "subscribe: %v", err)

	// 2. read the ack – blocking read just once
	var a rpcResponse
	require.NoError(t, conn.ReadJSON(&a), "ack: %v", err)
	t.Log("subscribed to websocket - id=", string(a.Result))

	output := make([]Out, 0)

	// Function to handle JSON reading with error channel
	readJSON := func(conn *websocket.Conn, msg *json.RawMessage) <-chan error {
		errChan := make(chan error, 1) // Buffered channel to avoid goroutine leak

		go func() {
			errChan <- conn.ReadJSON(msg)
			close(errChan)
		}()

		return errChan
	}

	var msg json.RawMessage

	// 3. start a goroutine that keeps reading pushes
outer_loop:
	for {
		select {
		case <-runUntil:
			// Clean‑up if necessary, then exit
			t.Log(method, "subscriber", "stopping: runUntil condition met")
			break outer_loop
		case <-t.Ctx().Done():
			// Clean‑up if necessary, then exit
			t.Log("unsafe head subscriber", "stopping: context cancelled")
			break outer_loop
		case err := <-readJSON(conn, &msg):
			require.NoError(t, err, "read: %v", err)

			var p push[Out]
			require.NoError(t, json.Unmarshal(msg, &p), "decode: %v", err)

			t.Log(wsRPC, method, "received websocket message", p.Params.Result)
			output = append(output, p.Params.Result)
		}
	}

	require.NoError(t, conn.WriteJSON(rpcRequest{
		JSONRPC: "2.0",
		ID:      2,
		Method:  prefix + "_unsubscribe_" + method,
		Params:  []any{a.Result},
	}), "unsubscribe: %v", err)

	t.Log("gracefully closed websocket connection")

	return output
}

func GetKonaWs[T any](t devtest.T, wsRPC string, method string, runUntil <-chan T) []eth.L2BlockRef {
	return GetPrefixedWs[T, eth.L2BlockRef](t, "ws", wsRPC, method, runUntil)
}

func GetDevWS[T any](t devtest.T, wsRPC string, method string, runUntil <-chan T) []uint64 {
	return GetPrefixedWs[T, uint64](t, "dev", wsRPC, method, runUntil)
}
