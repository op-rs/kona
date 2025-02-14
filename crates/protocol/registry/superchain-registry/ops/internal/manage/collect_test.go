package manage

import (
	"testing"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
	"github.com/stretchr/testify/require"
)

func TestCollectChainConfigs(t *testing.T) {
	p := paths.SuperchainDir("testdata", "sepolia")

	chains, err := CollectChainConfigs(p)
	require.NoError(t, err)
	require.Len(t, chains, 2)

	var opConfig config.Chain
	var testChainConfig config.Chain
	require.NoError(t, paths.ReadTOMLFile(paths.ChainConfig("testdata", "sepolia", "op"), &opConfig))
	require.NoError(t, paths.ReadTOMLFile(paths.ChainConfig("testdata", "sepolia", "testchain"), &testChainConfig))

	require.Equal(t, []DiskChainConfig{
		{
			ShortName: "op",
			Filepath:  paths.ChainConfig("testdata", "sepolia", "op"),
			Config:    &opConfig,
		},
		{
			ShortName: "testchain",
			Filepath:  paths.ChainConfig("testdata", "sepolia", "testchain"),
			Config:    &testChainConfig,
		},
	}, chains)
}
