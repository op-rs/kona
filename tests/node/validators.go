package node

import (
	"context"
	"errors"
	"strings"

	"github.com/ethereum-optimism/optimism/devnet-sdk/system"
	"github.com/ethereum-optimism/optimism/devnet-sdk/testing/systest"
	"github.com/ethereum-optimism/optimism/op-service/apis"
	"github.com/stretchr/testify/require"
)

func isKonaNode(t systest.T, clRPC string, clName string) bool {
	peerInfo := &apis.PeerInfo{}

	require.NoError(t, SendRPCRequest(clRPC, "opp2p_self", peerInfo), "failed to send RPC request to node %s: %s", clName)

	// For now, the ws endpoint is only supported by kona nodes.
	if !strings.Contains(strings.ToLower(peerInfo.UserAgent), "kona") {
		return false
	}

	return true
}

func atLeastOneNodeSupportsKonaWs(t systest.T, sys system.System) (context.Context, error) {
	l2s := sys.L2s()
	for _, l2 := range l2s {
		for _, node := range l2.Nodes() {
			clRPC := node.CLRPC()
			clName := node.CLName()
			if isKonaNode(t, clRPC, clName) {
				return t.Context(), nil
			}
		}
	}
	return nil, errors.New("no node supports ws endpoint")
}

func sysHasAtMostOneL1Node(t systest.T, sys system.System) (context.Context, error) {
	l1s := sys.L1().Nodes()
	if len(l1s) > 1 {
		return nil, errors.New("the network has more than one L1 node")
	}
	return t.Context(), nil
}
