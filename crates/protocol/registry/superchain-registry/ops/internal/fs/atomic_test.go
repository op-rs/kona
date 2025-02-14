package fs

import (
	"io"
	"os"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestAtomicWrite(t *testing.T) {
	f, err := os.CreateTemp("", "atomic-test")
	require.NoError(t, err)
	t.Cleanup(func() {
		require.NoError(t, f.Close())
		require.NoError(t, os.Remove(f.Name()))
	})

	expData := []byte("hello, world")
	require.NoError(t, AtomicWrite(f.Name(), 0o755, expData))

	// Original handle is replaced by the atomic move, so it should be empty
	originalHandleData, err := io.ReadAll(f)
	require.NoError(t, err)
	require.Empty(t, originalHandleData)

	actData, err := os.ReadFile(f.Name())
	require.NoError(t, err)
	require.EqualValues(t, expData, actData)
}
