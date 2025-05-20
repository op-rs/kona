package node

import (
	"testing"

	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/testlib/validators"
)

// Contains general system tests for the node.
func TestSystemKonaTest(t *testing.T) {
	// Get the L2 chain we want to test with
	chainIdx := uint64(0) // First L2 chain

	nodeValidator := validators.HasSufficientL2Nodes(chainIdx, 2)

	systest.SystemTest(t,
		peerCount(),
		nodeValidator,
	)
}
