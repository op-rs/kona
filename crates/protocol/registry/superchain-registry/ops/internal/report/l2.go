package report

import (
	"errors"
	"fmt"
	"math/big"
	"os"

	"github.com/ethereum-optimism/optimism/op-chain-ops/foundry"
	"github.com/ethereum-optimism/optimism/op-chain-ops/genesis"
	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/broadcaster"
	opcmv170 "github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/opcm/v170"
	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/state"
	"github.com/ethereum-optimism/optimism/op-deployer/pkg/env"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/validation"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/log"
)

func ScanL2(
	startBlock *types.Header,
	chainCfg *config.StagedChain,
	originalGenesis *core.Genesis,
	afacts foundry.StatDirFs,
) (*L2Report, error) {
	var report L2Report

	if !chainCfg.DeploymentL2ContractsVersion.Canonical {
		return nil, errors.New("contracts version is not canonical")
	}

	report.Release = chainCfg.DeploymentL2ContractsVersion.Tag
	report.ProvidedGenesisHash = originalGenesis.ToBlock().Hash()

	standardGenesisHash, diffs, err := DiffL2Genesis(chainCfg, originalGenesis.Alloc, afacts, startBlock)
	if err != nil {
		return nil, fmt.Errorf("failed to diff L2 genesis: %w", err)
	}

	report.StandardGenesisHash = standardGenesisHash
	report.AccountDiffs = diffs
	return &report, nil
}

func DiffL2Genesis(
	chainCfg *config.StagedChain,
	originalAllocs types.GenesisAlloc,
	artifacts foundry.StatDirFs,
	startBlock *types.Header,
) (common.Hash, []AccountDiff, error) {
	var standardHash common.Hash

	var l1ChainID uint64
	var standardRoles validation.RolesConfig
	if chainCfg.Superchain == config.MainnetSuperchain {
		standardRoles = validation.StandardConfigRolesMainnet
		l1ChainID = 1
	} else if chainCfg.Superchain == config.SepoliaSuperchain {
		standardRoles = validation.StandardConfigRolesSepolia
		l1ChainID = 11155111
	} else {
		return standardHash, nil, fmt.Errorf("unsupported superchain: %s", chainCfg.Superchain)
	}

	standardIntent := &state.Intent{
		ConfigType:         state.IntentConfigTypeStrict,
		FundDevAccounts:    false,
		UseInterop:         false,
		L1ContractsLocator: chainCfg.DeploymentL1ContractsVersion,
		L2ContractsLocator: chainCfg.DeploymentL2ContractsVersion,
		L1ChainID:          l1ChainID,

		SuperchainRoles: &state.SuperchainRoles{
			ProxyAdminOwner:       common.Address(standardRoles.L1ProxyAdminOwner),
			Guardian:              common.Address(standardRoles.Guardian),
			ProtocolVersionsOwner: common.Address(standardRoles.ProtocolVersionsOwner),
		},

		Chains: []*state.ChainIntent{
			{
				ID:                         common.BigToHash(new(big.Int).SetUint64(chainCfg.ChainID)),
				BaseFeeVaultRecipient:      common.Address(chainCfg.BaseFeeVaultRecipient),
				L1FeeVaultRecipient:        common.Address(chainCfg.L1FeeVaultRecipient),
				SequencerFeeVaultRecipient: common.Address(chainCfg.SequencerFeeVaultRecipient),
				Eip1559DenominatorCanyon:   chainCfg.Optimism.EIP1559DenominatorCanyon,
				Eip1559Denominator:         chainCfg.Optimism.EIP1559Denominator,
				Eip1559Elasticity:          chainCfg.Optimism.EIP1559Elasticity,
				Roles: state.ChainRoles{
					L1ProxyAdminOwner: common.Address(standardRoles.L1ProxyAdminOwner),
					L2ProxyAdminOwner: common.Address(standardRoles.L2ProxyAdminOwner),
					SystemConfigOwner: common.Address(*chainCfg.Roles.SystemConfigOwner),
					UnsafeBlockSigner: common.Address(*chainCfg.Roles.UnsafeBlockSigner),
					Batcher:           common.Address(*chainCfg.Roles.BatchSubmitter),
					Proposer:          common.Address(*chainCfg.Roles.Proposer),
					Challenger:        common.Address(*chainCfg.Roles.Challenger),
				},
			},
		},
	}

	// Hack until I find a better way of doing this
	if chainCfg.DeploymentL1ContractsVersion.Tag == string(validation.Semver160) {
		standardIntent.GlobalDeployOverrides = map[string]any{
			"l2GenesisHoloceneTimeOffset": nil,
		}
	}

	standardState := &state.State{
		// These values are not used in L2 genesis.
		SuperchainDeployment: new(state.SuperchainDeployment),
	}

	standardChainState := &state.ChainState{
		ID:                                 common.BigToHash(new(big.Int).SetUint64(chainCfg.ChainID)),
		StartBlock:                         startBlock,
		L1StandardBridgeProxyAddress:       common.Address(*chainCfg.Addresses.L1StandardBridgeProxy),
		L1CrossDomainMessengerProxyAddress: common.Address(*chainCfg.Addresses.L1CrossDomainMessengerProxy),
		L1ERC721BridgeProxyAddress:         common.Address(*chainCfg.Addresses.L1ERC721BridgeProxy),
		SystemConfigProxyAddress:           common.Address(*chainCfg.Addresses.SystemConfigProxy),
		OptimismPortalProxyAddress:         common.Address(*chainCfg.Addresses.OptimismPortalProxy),
	}

	standardDeployConfig, err := state.CombineDeployConfig(standardIntent, standardIntent.Chains[0], standardState, standardChainState)
	if err != nil {
		return standardHash, nil, fmt.Errorf("failed to combine deploy config: %w", err)
	}

	lgr := log.NewLogger(log.NewTerminalHandlerWithLevel(os.Stderr, log.LevelWarn, false))
	deployer := common.Address{'D'}
	host, err := env.DefaultScriptHost(
		broadcaster.NoopBroadcaster(),
		lgr,
		deployer,
		artifacts,
	)
	if err != nil {
		return standardHash, nil, fmt.Errorf("failed to create script host: %w", err)
	}

	if err := opcmv170.L2Genesis(host, &opcmv170.L2GenesisInput{
		L1Deployments: opcmv170.L1Deployments{
			L1CrossDomainMessengerProxy: common.Address(*chainCfg.Addresses.L1CrossDomainMessengerProxy),
			L1StandardBridgeProxy:       common.Address(*chainCfg.Addresses.L1StandardBridgeProxy),
			L1ERC721BridgeProxy:         common.Address(*chainCfg.Addresses.L1ERC721BridgeProxy),
		},
		L2Config: standardDeployConfig.L2InitializationConfig,
	}); err != nil {
		return standardHash, nil, fmt.Errorf("failed to call v170 L2Genesis script: %w", err)
	}

	host.Wipe(deployer)

	standardAllocs, err := host.StateDump()
	if err != nil {
		return standardHash, nil, fmt.Errorf("failed to dump state: %w", err)
	}

	standardGenesis, err := genesis.BuildL2Genesis(&standardDeployConfig, standardAllocs, startBlock)
	if err != nil {
		return standardHash, nil, fmt.Errorf("failed to build standard genesis: %w", err)
	}

	diffs := DiffAllocs(standardAllocs.Accounts, originalAllocs)
	return standardGenesis.ToBlock().Hash(), diffs, nil
}
