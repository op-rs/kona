package node

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"net/http"
	"time"

	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/gorilla/websocket"
	"github.com/stretchr/testify/require"
)

// --- Generic RPC request/response types -------------------------------------

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

// ---------------------------------------------------------------------------

const (
	DEFAULT_TIMEOUT = 10 * time.Second
)

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

	err = json.Unmarshal(rpcResp.Result, resOutput)

	if err != nil {
		return err
	}

	return nil

}

func GetKonaWS[T any](t systest.T, wsRPC string, method string, runUntil <-chan T) []eth.L2BlockRef {
	conn, _, err := websocket.DefaultDialer.DialContext(t.Context(), wsRPC, nil)
	require.NoError(t, err, "dial: %v", err)
	defer conn.Close()

	// 1. send the *_subscribe request
	require.NoError(t, conn.WriteJSON(rpcRequest{
		JSONRPC: "2.0",
		ID:      1,
		Method:  "ws_subscribe_" + method,
		Params:  nil,
	}), "subscribe: %v", err)

	// 2. read the ack – blocking read just once
	var a rpcResponse
	require.NoError(t, conn.ReadJSON(&a), "ack: %v", err)
	t.Log("subscribed to websocket - id=", string(a.Result))

	output := make([]eth.L2BlockRef, 0)

	// 3. start a goroutine that keeps reading pushes
outer_loop:
	for {
		select {
		case <-runUntil:
			// Clean‑up if necessary, then exit
			t.Log(method, "subscriber", "stopping: runUntil condition met")
			break outer_loop
		case <-t.Context().Done():
			// Clean‑up if necessary, then exit
			t.Log("unsafe head subscriber", "stopping: context cancelled")
			break outer_loop
		default:
			var msg json.RawMessage
			require.NoError(t, conn.ReadJSON(&msg), "read: %v", err)

			var p push
			require.NoError(t, json.Unmarshal(msg, &p), "decode: %v", err)

			t.Log("received websocket message - ", p.Params.Result)
			output = append(output, p.Params.Result)
		}
	}

	require.NoError(t, conn.WriteJSON(rpcRequest{
		JSONRPC: "2.0",
		ID:      2,
		Method:  "ws_unsubscribe_" + method,
		Params:  []interface{}{a.Result},
	}), "unsubscribe: %v", err)

	t.Log("gracefully closed websocket connection")

	return output
}
