package node

import (
	"testing"

	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/testlib/validators"
)

// Contains general system tests for the p2p connectivity of the node.
// This assumes there is at least two L2 chains. The second chain is used to test a larger network.
func TestSystemNodeP2p(t *testing.T) {
	// Check that the node has at least 1 peer that is connected to its topics when there is more than 1 peer in the network.
	systest.SystemTest(t,
		peerCount(1, 1),
		validators.HasSufficientL2Nodes(0, 2),
	)

	systest.SystemTest(t,
		allPeersInNetwork(),
	)

	// Check that the node has at least 2 peers that are connected to its topics when there is more than 3 peers in the network initially.
	// We put a lower bound on the number of connected peers to account for network instability.
	systest.SystemTest(t,
		peerCount(2, 2),
		validators.HasSufficientL2Nodes(0, 3),
	)

}
