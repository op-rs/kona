package node

import (
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/testlib/validators"
)

// Contains general system tests for the p2p connectivity of the node.
// This assumes there is at least two L2 chains. The second chain is used to test a larger network.
func TestSystemNodeP2p(t *testing.T) {
	t.Parallel()

	systest.SystemTest(t,
		allPeersInNetwork(),
		validators.HasSufficientL2Nodes(0, 2),
	)

	// Check that the node has at least 1 peer that is connected to its topics when there is more than 1 peer in the network.
	systest.SystemTest(t,
		peerCount(1, 1),
		validators.HasSufficientL2Nodes(0, 2),
	)
}

// Check that the node has at least 4 peers that are connected to its topics when there is more than 6 peers in the network initially.
func TestSystemNodeP2pLargeNetwork(t *testing.T) {
	t.Parallel()
	// Wait for the network to stabilize.
	time.Sleep(2 * time.Minute)

	systest.SystemTest(t,
		peerCount(4, 5),
		validators.HasSufficientL2Nodes(0, 6),
	)
}

// Ensures that the node synchronizes the chain correctly.
func TestSystemNodeFinalizedSync(t *testing.T) {
	t.Parallel()

	systest.SystemTest(t,
		syncFinalized(),
		atLeastOneNodeSupportsKonaWs,
	)
}

// Ensures that the node synchronizes the safe chain correctly.
func TestSystemSyncSafe(t *testing.T) {
	t.Parallel()

	systest.SystemTest(t,
		syncSafe(),
		atLeastOneNodeSupportsKonaWs,
		sysHasAtMostOneL1Node,
	)
}

// Ensures that the unsafe chain eventually becomes the safe chain.
func TestSystemSyncUnsafeBecomesSafe(t *testing.T) {
	t.Parallel()

	systest.SystemTest(t,
		syncUnsafeBecomesSafe(),
		atLeastOneNodeSupportsKonaWs,
		sysHasAtMostOneL1Node,
	)
}

// Ensures that the node synchronizes the unsafe chain correctly.
func TestSystemSyncUnsafe(t *testing.T) {
	t.Parallel()

	systest.SystemTest(t,
		syncUnsafe(),
		atLeastOneNodeSupportsKonaWs,
		sysHasAtMostOneL1Node,
	)
}
