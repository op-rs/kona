package node_utils

import (
	"context"
	"fmt"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-service/client"
	"github.com/kurtosis-tech/kurtosis/api/golang/engine/lib/kurtosis_context"
)

const DefaultL1ID = 900
const DefaultL2ID = 901

// --- Generic RPC request/response types -------------------------------------

// ---------------------------------------------------------------------------

const (
	DEFAULT_TIMEOUT = 10 * time.Second
)

// rpcEndpoint gets the RPC endpoint URL for a specified service from Kurtosis
func rpcEndpoint(ctx context.Context, serviceName string) (string, error) {
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

func GetNodeRPCEndpoint(node *dsl.L2CLNode) client.RPC {
	return node.Escape().ClientRPC()
}

func SendRPCRequest[T any](clientRPC client.RPC, method string, resOutput *T, params ...any) error {
	ctx, cancel := context.WithTimeout(context.Background(), DEFAULT_TIMEOUT)
	defer cancel()

	return clientRPC.CallContext(ctx, &resOutput, method, params...)
}
