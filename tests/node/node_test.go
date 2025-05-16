package node

import (
	"testing"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/testlib/validators"
)

func nodeUpScenario() systest.SystemTestFunc {
	return func(t systest.T, sys system.System) {
		l2s := sys.L2s()

		for _, l2 := range l2s {
			config, err := l2.Config()
			if err != nil {
				t.Log(err)
			}

			t.Log(l2.ID(), l2.Nodes(), config)
		}
	}
}

func TestSystemKonaTest(t *testing.T) {
	// Get the L2 chain we want to test with
	chainIdx := uint64(0) // First L2 chain

	nodeValidator := validators.HasSufficientL2Nodes(chainIdx, 1)

	systest.SystemTest(t,
		nodeUpScenario(),
		nodeValidator,
	)
}
