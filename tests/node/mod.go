package node

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"net/http"
	"time"
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

func SendRPCRequest(addr string, method string, params ...any) (rpcResponse, error) {
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
		return (rpcResponse{}), err
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
		return (rpcResponse{}), err
	}
	req.Header.Set("Content-Type", "application/json")

	// 5. Send the request.
	resp, err := client.Do(req)
	if err != nil {
		return (rpcResponse{}), err
	}
	defer resp.Body.Close()

	// 6. Read and decode the response.
	respBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return (rpcResponse{}), err
	}

	var rpcResp rpcResponse
	if err := json.Unmarshal(respBytes, &rpcResp); err != nil {
		return (rpcResponse{}), err
	}

	return rpcResp, nil

}
