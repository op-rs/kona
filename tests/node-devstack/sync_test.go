package nodedevstack

import (
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/stretchr/testify/require"
)

func TestL2Setup(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewSimpleKona(t)
	info := out.L2CLKona.PeerInfo()

	require.Equal(t, info.UserAgent, "kona")
}
