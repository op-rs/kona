package supervisor

// todo: add tests
import (
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-devstack/presets"
)

// TestMain creates the test-setups against the shared backend
func TestMain(m *testing.M) {
	// sleep to ensure the backend is ready
	time.Sleep(90 * time.Second)

	presets.DoMain(m, presets.WithSimpleInterop())
}
