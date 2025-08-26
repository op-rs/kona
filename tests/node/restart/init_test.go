package node_restart

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	node_utils "github.com/op-rs/kona/node/utils"
)

// TestMain creates the test-setups against the shared backend
func TestMain(m *testing.M) {
	presets.DoMain(m, node_utils.WithMixedOpKona(0, 1, 0, 2))
}
