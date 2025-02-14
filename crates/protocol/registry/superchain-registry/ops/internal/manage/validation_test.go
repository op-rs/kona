package manage

import (
	"testing"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core"
	"github.com/stretchr/testify/require"
)

func TestValidateGenesisIntegrity(t *testing.T) {
	t.Run("validates genesis successfully", func(t *testing.T) {
		t.Parallel()

		cfg, err := ReadChainConfig("testdata", "sepolia", "testchain")
		require.NoError(t, err)

		genesis, err := ReadSuperchainGenesis("testdata", "sepolia", "testchain")
		require.NoError(t, err)

		err = ValidateGenesisIntegrity(cfg, genesis)
		require.NoError(t, err)
	})

	type testCase struct {
		name    string
		mutator func(*core.Genesis, *config.Chain)
	}

	tests := []testCase{
		{
			name: "fails when the hash is wrong in the config",
			mutator: func(_ *core.Genesis, c *config.Chain) {
				c.Genesis.L2.Hash = common.HexToHash("0x1234")
			},
		},
		{
			name: "fails when timestamp is modified",
			mutator: func(g *core.Genesis, _ *config.Chain) {
				g.Timestamp++
			},
		},
		{
			name: "fails when nonce is modified",
			mutator: func(g *core.Genesis, _ *config.Chain) {
				g.Nonce++
			},
		},
		{
			name: "fails when extradata is modified",
			mutator: func(g *core.Genesis, _ *config.Chain) {
				g.ExtraData = append(g.ExtraData, byte(0x00))
			},
		},
		{
			name: "fails when gas limit is modified",
			mutator: func(g *core.Genesis, _ *config.Chain) {
				g.GasLimit++
			},
		},
		{
			name: "fails when base fee is modified",
			mutator: func(g *core.Genesis, _ *config.Chain) {
				g.BaseFee.Add(g.BaseFee, common.Big1)
			},
		},
		{
			name: "fails when blob gas parameters are modified",
			mutator: func(g *core.Genesis, _ *config.Chain) {
				g.ExcessBlobGas = new(uint64)
				*g.ExcessBlobGas = 1
			},
		},
		{
			name: "fails when state hash is modified",
			mutator: func(g *core.Genesis, _ *config.Chain) {
				g.StateHash = new(common.Hash)
				g.StateHash[0] = 0x01
				g.Alloc = nil
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			cfg, err := ReadChainConfig("testdata", "sepolia", "testchain")
			require.NoError(t, err)

			genesis, err := ReadSuperchainGenesis("testdata", "sepolia", "testchain")
			require.NoError(t, err)

			tt.mutator(genesis, cfg)

			err = ValidateGenesisIntegrity(cfg, genesis)
			require.Error(t, err)
			require.ErrorContains(t, err, "genesis hash mismatch")
		})
	}
}
