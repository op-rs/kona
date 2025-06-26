package nodedevstack

import (
	"context"
	"fmt"
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/kurtosis-tech/kurtosis/api/golang/engine/lib/kurtosis_context"
	"github.com/stretchr/testify/require"
)

// GetCPUStats executes shell commands to get CPU usage statistics from a service
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

func GetNodeRPCEndpoint(ctx context.Context, node *dsl.L2CLNode) (string, error) {
	return rpcEndpoint(ctx, node.Escape().ID().Key())
}

func TestRPCEndpoints(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	for _, node := range out.L2CLKonaNodes {
		endpoint, err := GetNodeRPCEndpoint(t.Ctx(), &node)
		require.NoError(t, err, "failed to get RPC endpoint for node %s", node.Escape().ID().Key())

		t.Logf("RPC endpoint for node %s: %s", node.Escape().ID().Key(), endpoint)
	}
}
