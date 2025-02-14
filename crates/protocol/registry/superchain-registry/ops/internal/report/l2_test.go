package report

import (
	"bytes"
	"compress/gzip"
	"context"
	"encoding/json"
	"math/big"
	"os"
	"path"
	"testing"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/artifacts"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/validation"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/stretchr/testify/require"
)

func TestScanL2(t *testing.T) {
	chainCfgData := readTestData(t, "chain-config.toml")
	genesisData := readTestData(t, "genesis.json.gz")
	startBlockData := readTestData(t, "start-block.json")

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	afacts, cleanup, err := artifacts.Download(ctx, artifacts.MustNewLocatorFromURL("tag://"+string(validation.Semver170)), artifacts.NoopDownloadProgressor)
	require.NoError(t, err)
	t.Cleanup(func() {
		require.NoError(t, cleanup())
	})

	testAddr := common.HexToAddress("0x4200000000000000000000000000000000000000")

	tests := []struct {
		name       string
		setup      func(*types.Header, *config.StagedChain, *core.Genesis)
		wantErr    string
		wantReport L2Report
	}{
		{
			name: "successful scan",
			setup: func(*types.Header, *config.StagedChain, *core.Genesis) {
			},
			wantReport: L2Report{
				Release:             string(validation.Semver170),
				ProvidedGenesisHash: common.HexToHash("0xcd901673f97d59259fa09b0b01b8787f5d25d9f1808566990673519be65cc3ae"),
				StandardGenesisHash: common.HexToHash("0xcd901673f97d59259fa09b0b01b8787f5d25d9f1808566990673519be65cc3ae"),
				AccountDiffs:        []AccountDiff{},
			},
		},
		{
			name: "non-canonical L2 contracts locator",
			setup: func(_ *types.Header, sc *config.StagedChain, _ *core.Genesis) {
				sc.DeploymentL2ContractsVersion.Canonical = false
			},
			wantErr: "contracts version is not canonical",
		},
		{
			name: "different account balance",
			setup: func(_ *types.Header, sc *config.StagedChain, genesis *core.Genesis) {
				account := genesis.Alloc[testAddr]
				*account.Balance = *big.NewInt(999)
			},
			wantReport: L2Report{
				Release:             string(validation.Semver170),
				ProvidedGenesisHash: common.HexToHash("0x42c3817d6176e7764ad7920049859cd97fe217394c3f45171355b8d5b392ae52"),
				StandardGenesisHash: common.HexToHash("0xcd901673f97d59259fa09b0b01b8787f5d25d9f1808566990673519be65cc3ae"),
				AccountDiffs: []AccountDiff{
					{
						Address:        testAddr,
						Added:          false,
						Removed:        false,
						BalanceChanged: true,

						OldBalance: big.NewInt(0),
						NewBalance: big.NewInt(999),
					},
				},
			},
		},
		{
			name: "additional account in original",
			setup: func(_ *types.Header, sc *config.StagedChain, genesis *core.Genesis) {
				genesis.Alloc[common.HexToAddress("0x111")] = types.Account{
					Balance: big.NewInt(999),
					Code:    []byte{0x01},
					Nonce:   1,
				}
			},
			wantReport: L2Report{
				Release:             string(validation.Semver170),
				ProvidedGenesisHash: common.HexToHash("0x233c30d682b8c318aa6a8be73e4123076b40618b9943f517f93c67eecd115319"),
				StandardGenesisHash: common.HexToHash("0xcd901673f97d59259fa09b0b01b8787f5d25d9f1808566990673519be65cc3ae"),
				AccountDiffs: []AccountDiff{
					{
						Address:        common.HexToAddress("0x111"),
						Added:          true,
						CodeChanged:    true,
						BalanceChanged: true,
						NonceChanged:   true,
						NewCode:        []byte{0x01},
						NewBalance:     big.NewInt(999),
						NewNonce:       1,
					},
				},
			},
		},
		{
			name: "different account code",
			setup: func(_ *types.Header, sc *config.StagedChain, genesis *core.Genesis) {
				account := genesis.Alloc[testAddr]
				genesis.Alloc[testAddr] = types.Account{
					Balance: account.Balance,
					Code:    []byte{0x42, 0x42},
					Nonce:   account.Nonce,
					Storage: account.Storage,
				}
			},
			wantReport: L2Report{
				Release:             string(validation.Semver170),
				ProvidedGenesisHash: common.HexToHash("0x34ef89a965ff95832f1de620ec385fc4191f695dfffe20ca1595f33586f24374"),
				StandardGenesisHash: common.HexToHash("0xcd901673f97d59259fa09b0b01b8787f5d25d9f1808566990673519be65cc3ae"),
				AccountDiffs: []AccountDiff{
					{
						Address:     testAddr,
						Added:       false,
						Removed:     false,
						CodeChanged: true,
						OldCode:     readTestData(t, "proxy.bin"),
						NewCode:     []byte{0x42, 0x42},
					},
				},
			},
		},
		{
			name: "different storage values",
			setup: func(_ *types.Header, sc *config.StagedChain, genesis *core.Genesis) {
				addr := common.HexToAddress("0x4200000000000000000000000000000000000043")
				account := genesis.Alloc[addr]
				account.Storage[common.HexToHash("0xabcd")] = common.HexToHash("0x1234")
			},
			wantReport: L2Report{
				Release:             string(validation.Semver170),
				ProvidedGenesisHash: common.HexToHash("0xf80d466dc95792601043a3d769d3d018e0cf5a71386cceab07cc2cd4a94a5f55"),
				StandardGenesisHash: common.HexToHash("0xcd901673f97d59259fa09b0b01b8787f5d25d9f1808566990673519be65cc3ae"),
				AccountDiffs: []AccountDiff{
					{
						Address: common.HexToAddress("0x4200000000000000000000000000000000000043"),

						StorageChanges: []StorageDiff{
							{
								Key:      common.HexToHash("0xabcd"),
								Added:    true,
								Removed:  false,
								NewValue: common.HexToHash("0x1234"),
							},
						},
					},
				},
			},
		},
	}

	for _, tt := range tests {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			var chainCfg config.StagedChain
			require.NoError(t, toml.Unmarshal(chainCfgData, &chainCfg))
			gzr, err := gzip.NewReader(bytes.NewReader(genesisData))
			require.NoError(t, err)
			var genesis core.Genesis
			require.NoError(t, json.NewDecoder(gzr).Decode(&genesis))
			require.NoError(t, gzr.Close())
			var startBlock types.Header
			require.NoError(t, json.Unmarshal(startBlockData, &startBlock))

			tt.setup(&startBlock, &chainCfg, &genesis)
			report, err := ScanL2(&startBlock, &chainCfg, &genesis, afacts)

			if tt.wantErr == "" {
				require.NoError(t, err)
				require.Equal(t, tt.wantReport, *report)
			} else {
				require.ErrorContains(t, err, tt.wantErr)
				require.Nil(t, report)
			}
		})
	}
}

func readTestData(t *testing.T, fname string) []byte {
	data, err := os.ReadFile(path.Join("testdata", fname))
	require.NoError(t, err)
	return data
}
