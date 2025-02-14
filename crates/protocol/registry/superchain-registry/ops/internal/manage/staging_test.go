package manage

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-chain-ops/genesis"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/stretchr/testify/require"
)

func TestCopyDeployConfigHFTimes(t *testing.T) {
	a := &genesis.UpgradeScheduleDeployConfig{
		L2GenesisCanyonTimeOffset: new(hexutil.Uint64),
		L2GenesisDeltaTimeOffset:  new(hexutil.Uint64),
	}
	*a.L2GenesisCanyonTimeOffset = hexutil.Uint64(1)
	*a.L2GenesisDeltaTimeOffset = hexutil.Uint64(2)

	b := &config.Hardforks{}

	require.NoError(t, CopyDeployConfigHFTimes(a, b))
	require.Equal(t, &config.Hardforks{
		CanyonTime: config.NewHardforkTime(1),
		DeltaTime:  config.NewHardforkTime(2),
	}, b)
}
