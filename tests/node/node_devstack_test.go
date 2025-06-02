package node

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/stretchr/testify/require"
)

func TestMain(t *testing.M) {
	presets.DoMain(t, presets.WithMinimal())
}

func TestSystemNodeP2pDevstack(gt *testing.T) {
	t := devtest.ParallelT(gt)
	sys := presets.NewMinimal(t)

	peerInfo := sys.L2CL.PeerInfo()

	require.NotNil(t, peerInfo)

	t.Log(peerInfo)
}
