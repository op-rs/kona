package node_sysgo

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	kona_presets "github.com/op-rs/kona/node/presets"
)

// TestMain creates the test-setups against the shared backend
func TestMain(m *testing.M) {
	presets.DoMain(m, kona_presets.WithMixedOpKona(0, 1, 0, 2))
}
