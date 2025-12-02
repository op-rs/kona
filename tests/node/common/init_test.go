package node

import (
	"fmt"
	"os"
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	node_utils "github.com/op-rs/kona/node/utils"
)

// TestMain creates the test-setups against the shared backend
func TestMain(m *testing.M) {
	config := node_utils.ParseL2NodeConfigFromEnv()
	fmt.Printf("Running e2e tests with Config: %d\n", config)

	// Use conductor-compatible preset when running with Kurtosis (sysext)
	if os.Getenv("DEVSTACK_ORCHESTRATOR") == "sysext" {
		presets.DoMain(m, node_utils.WithMixedOpKonaWithConductors(config))
	} else {
		presets.DoMain(m, node_utils.WithMixedOpKona(config))
	}
}
