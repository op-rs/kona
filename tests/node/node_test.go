package node

import (
	"testing"

	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
)

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
