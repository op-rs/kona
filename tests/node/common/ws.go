package node

import (
	"encoding/json"
	"strings"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/gorilla/websocket"
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

// ---------------------------------------------------------------------------

func GetPrefixedWs[T any, Out any](t devtest.T, node *dsl.L2CLNode, prefix string, method string, runUntil <-chan T) []Out {
	userRPC := node.Escape().UserRPC()
	wsRPC := strings.Replace(userRPC, "http", "ws", 1)

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

func GetKonaWs[T any](t devtest.T, node *dsl.L2CLNode, method string, runUntil <-chan T) []eth.L2BlockRef {
	return GetPrefixedWs[T, eth.L2BlockRef](t, node, "ws", method, runUntil)
}

func GetDevWS[T any](t devtest.T, node *dsl.L2CLNode, method string, runUntil <-chan T) []uint64 {
	return GetPrefixedWs[T, uint64](t, node, "dev", method, runUntil)
}
