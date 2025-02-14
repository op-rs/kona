package report

import (
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"math/big"
	"os"
	"path"
	"testing"

	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/artifacts"
	"github.com/ethereum-optimism/optimism/op-service/testlog"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/testutil/mockrpc"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/rpc"
	"github.com/stretchr/testify/require"
)

func TestParseDeployedEvent(t *testing.T) {
	// Taken from the deployment at https://etherscan.io/tx/0x18c55303075270503bec79e66c444c15d943598f25fbf467044b3c5dda9e7d58.
	rawLogF, err := os.Open("testdata/tx-18c55303075270503bec79e66c444c15d943598f25fbf467044b3c5dda9e7d58.bin")
	require.NoError(t, err)
	defer rawLogF.Close()

	rawLog, err := io.ReadAll(rawLogF)
	require.NoError(t, err)

	log := &types.Log{
		Data: rawLog,
		Topics: []common.Hash{
			common.HexToHash("0x9fbdf97c6b496bf20189c3c23d0640336fce48e18810c9b84558ec31de0ab9b0"),
			{},
			common.HexToHash("0x000000000000000000000000000000000000000000000000000000000001517d"),
			common.HexToHash("0x0000000000000000000000009cb6296f6c9b6bb5bf382e8c1ec82b7e373ec693"),
		},
	}

	deployedEvent, err := ParseDeployedEvent(log)
	require.NoError(t, err)

	marshal := func(in any) string {
		out, err := json.Marshal(in)
		require.NoError(t, err)
		return string(out)
	}

	// Have to use JSONEq here because require.EqualValues improperly marks
	// the structs an unequal.
	require.JSONEq(t, marshal(&DeployedEvent{
		OutputVersion: common.Big0,
		L2ChainID:     common.BigToHash(big.NewInt(0x1517d)),
		Deployer:      common.HexToAddress("0x9cb6296f6c9b6bb5bf382e8c1ec82b7e373ec693"),
		DeployOutput: DeployOPChainOutput{
			OpChainProxyAdmin:                  common.HexToAddress("0xb9a59B4dB790fD9674FFB03d38a2057dEAD4BC0F"),
			AddressManager:                     common.HexToAddress("0x2A1e2bE26e00E7f0f1D8d90240fC22F6f6C069AF"),
			L1ERC721BridgeProxy:                common.HexToAddress("0x775350Dc0DaCb54C86F723E17407B51Ae9B8a028"),
			SystemConfigProxy:                  common.HexToAddress("0x93dDE1822EfF3c3Bcdd7446c3E815EcF6952944c"),
			OptimismMintableERC20FactoryProxy:  common.HexToAddress("0x4964a238244F61cFa5F566335b8bAc194F9Af5DD"),
			L1StandardBridgeProxy:              common.HexToAddress("0x973a182df2479Ca1D9E12bFb7df9dEd485CC8DD7"),
			L1CrossDomainMessengerProxy:        common.HexToAddress("0xd40Ae4FC005C599142F4C4837D4781689eacca68"),
			OptimismPortalProxy:                common.HexToAddress("0xE87bF7d8D655e8f116A601fAA329CAC384463A0F"),
			DisputeGameFactoryProxy:            common.HexToAddress("0x2DB8b34CB5D9A798e4fCCa4c986f87DBa926DBed"),
			AnchorStateRegistryProxy:           common.HexToAddress("0x8808f36887e0455dAe01C4A68FD61dae4f06e470"),
			AnchorStateRegistryImpl:            common.HexToAddress("0x837F308824bCD43B615Bc4E1fda79a377103fb94"),
			FaultDisputeGame:                   common.HexToAddress("0x0000000000000000000000000000000000000000"),
			PermissionedDisputeGame:            common.HexToAddress("0x4253071492bF8BB5062568741259B3A887276334"),
			DelayedWETHPermissionedGameProxy:   common.HexToAddress("0x490479B4064c5380cd959709c7cA7229A39878Dc"),
			DelayedWETHPermissionlessGameProxy: common.HexToAddress("0x0000000000000000000000000000000000000000"),
		},
	}), marshal(deployedEvent))
}

func TestScanL1(t *testing.T) {
	releaseV160 := "op-contracts/v1.6.0"
	deploymentTx := common.HexToHash("0x18c55303075270503bec79e66c444c15d943598f25fbf467044b3c5dda9e7d58")

	mockedErrTest := func(t *testing.T, release string, p string, expErr string) {
		ctx, cancel := context.WithCancel(context.Background())
		defer cancel()

		client, mock := mockRPCClient(t, p)
		_, err := ScanL1(
			ctx,
			client,
			deploymentTx,
			&artifacts.Locator{Tag: release},
		)
		require.ErrorContains(t, err, expErr)
		mock.AssertExpectations(t)
	}

	t.Run("non-existent release", func(t *testing.T) {
		t.Parallel()
		mockedErrTest(t, "op-contracts/v99.0.0", "test-scan-l1-non-existent-release.json", "failed to get OPCM address")
	})

	tests := []struct {
		name     string
		filename string
		expErr   string
	}{
		{
			"non-existent tx",
			"test-scan-l1-non-existent-tx.json",
			"failed to get deployment receipt",
		},
		{
			"failed deployment tx",
			"test-scan-l1-failed-deployment-tx.json",
			"deployment tx failed",
		},
		{
			"unauthorized OPCM address",
			"test-scan-l1-unauthorized-opcm-address.json",
			"unauthorized address for Deployed event",
		},
		{
			"multiple deployed events in tx",
			"test-scan-l1-multiple-deployed-events.json",
			"multiple Deployed events in receipt",
		},
		{
			"malformed deployed event",
			"test-scan-l1-malformed-deployed-event.json",
			"malformed Deployed event",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			mockedErrTest(t, releaseV160, tt.filename, tt.expErr)
		})
	}
}

func TestScanSystemConfig(t *testing.T) {
	addr := common.HexToAddress("0x034edD2A225f7f429A63E0f1D2084B9E0A93b538")

	tests := []struct {
		name     string
		release  string
		filename string
		report   L1SystemConfigReport
	}{
		{
			"pre-holocene",
			"op-contracts/v1.6.0",
			"test-scan-systemconfig-pre-holocene.json",
			L1SystemConfigReport{
				GasLimit:               60000000,
				Scalar:                 big.NewInt(10),
				Overhead:               big.NewInt(20),
				IsGasPayingToken:       false,
				GasPayingToken:         common.HexToAddress("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
				GasPayingTokenDecimals: 18,
				GasPayingTokenName:     "Ether",
				GasPayingTokenSymbol:   "ETH",
			},
		},
		{
			"post-holocene",
			"op-contracts/v1.8.0-rc.4",
			"test-scan-systemconfig-post-holocene.json",
			L1SystemConfigReport{
				GasLimit:               60000000,
				Scalar:                 big.NewInt(1),
				Overhead:               big.NewInt(2),
				BaseFeeScalar:          3,
				BlobBaseFeeScalar:      4,
				EIP1559Denominator:     5,
				EIP1559Elasticity:      6,
				IsGasPayingToken:       false,
				GasPayingToken:         common.HexToAddress("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
				GasPayingTokenDecimals: 18,
				GasPayingTokenName:     "Ether",
				GasPayingTokenSymbol:   "ETH",
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			l1Client, mockRPC := mockRPCClient(t, tt.filename)
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()

			report, err := ScanSystemConfig(ctx, l1Client, tt.release, addr)
			require.NoError(t, err)
			require.EqualValues(t, tt.report, report)
			mockRPC.AssertExpectations(t)
		})
	}
}

func mockRPCClient(t *testing.T, p string) (*rpc.Client, *mockrpc.MockRPC) {
	mockRPC := mockrpc.NewMockRPC(
		t,
		testlog.Logger(t, slog.LevelWarn),
		mockrpc.WithExpectationsFile(t, path.Join("testdata", p)),
	)
	rpcClient, err := rpc.Dial(mockRPC.Endpoint())
	require.NoError(t, err)
	return rpcClient, mockRPC
}
