package preinterop

// todo: add tests
import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/presets"
)

// TestMain creates the test-setups against the shared backend
func TestMain(m *testing.M) {
	// sleep to ensure the backend is ready

	presets.DoMain(m,
		presets.WithSimpleInterop(),
		presets.WithInteropNotAtGenesis())
}
