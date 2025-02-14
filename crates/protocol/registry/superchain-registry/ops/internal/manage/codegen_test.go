package manage

import (
	"fmt"
	"os"
	"strings"
	"testing"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
	"github.com/stretchr/testify/require"
)

func TestGenAddressesFile(t *testing.T) {
	require.NoError(t, GenAddressesFile("testdata"))

	addrsFile := paths.AddressesFile("testdata")
	t.Cleanup(func() {
		require.NoError(t, os.Remove(addrsFile))
	})

	expected, err := os.ReadFile("testdata/expected-addresses.json")
	require.NoError(t, err)

	actual, err := os.ReadFile(addrsFile)
	require.NoError(t, err)

	require.JSONEq(t, string(expected), string(actual))
}

func TestGenChainList(t *testing.T) {
	for _, ext := range []string{"toml", "json"} {
		t.Run(ext, func(t *testing.T) {
			t.Parallel()

			outFile, err := os.CreateTemp("", fmt.Sprintf("chainList-*.%s", ext))
			require.NoError(t, err)
			defer outFile.Close()
			t.Cleanup(func() {
				require.NoError(t, os.Remove(outFile.Name()))
			})

			outPath := outFile.Name()
			require.NoError(t, GenChainListFile("testdata", outPath))
			actualBytes, err := os.ReadFile(outPath)
			require.NoError(t, err)
			expectedBytes, err := os.ReadFile(fmt.Sprintf("testdata/expected-chainList.%s", ext))
			require.NoError(t, err)
			require.Equal(t, strings.TrimSpace(string(expectedBytes)), strings.TrimSpace(string(actualBytes)))
		})
	}

	t.Run("any other extension", func(t *testing.T) {
		t.Parallel()

		err := GenChainListFile("testdata", "any-path.txt")
		require.ErrorContains(t, err, "unsupported file extension")
	})
}

func TestGenChainsReadme(t *testing.T) {
	readmeFile, err := os.CreateTemp("", "chains-*.md")
	require.NoError(t, err)
	t.Cleanup(func() {
		require.NoError(t, os.Remove(readmeFile.Name()))
	})

	require.NoError(t, GenChainsReadme("testdata", readmeFile.Name()))

	expectedBytes, err := os.ReadFile("testdata/expected-chains.md")
	require.NoError(t, err)

	actualBytes, err := os.ReadFile(readmeFile.Name())
	require.NoError(t, err)

	require.Equal(t, strings.TrimSpace(string(expectedBytes)), strings.TrimSpace(string(actualBytes)))
}
