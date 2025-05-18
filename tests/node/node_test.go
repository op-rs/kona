package node

import (
	"testing"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/testlib/validators"
	"github.com/stretchr/testify/require"
)

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

				clRPC := node.CLRPC()

				t.Log("CL RPC:", clRPC)

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
