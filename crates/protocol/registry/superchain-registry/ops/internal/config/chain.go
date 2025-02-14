package config

import (
	"encoding/json"
	"fmt"
	"reflect"
	"strings"

	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/artifacts"
	"github.com/ethereum-optimism/optimism/op-service/jsonutil"
	"github.com/ethereum/go-ethereum/common"
)

type SuperchainLevel int

const (
	SuperchainLevelNonStandard SuperchainLevel = iota
	SuperchainLevelStandardCandidate
	SuperchainLevelStandard
)

func NewSuperchainLevel(i int) (SuperchainLevel, error) {
	switch SuperchainLevel(i) {
	case SuperchainLevelNonStandard:
		return SuperchainLevelNonStandard, nil
	case SuperchainLevelStandardCandidate:
		return SuperchainLevelStandardCandidate, nil
	case SuperchainLevelStandard:
		return SuperchainLevelStandard, nil
	default:
		return SuperchainLevelNonStandard, fmt.Errorf("invalid superchain level: %d", i)
	}
}

func (s *SuperchainLevel) UnmarshalTOML(data any) error {
	switch i := data.(type) {
	case int64:
		lvl, err := NewSuperchainLevel(int(i))
		if err != nil {
			return fmt.Errorf("error unmarshaling superchain level: %w", err)
		}
		*s = lvl
		return nil
	default:
		return fmt.Errorf("invalid superchain level type: %T", data)
	}
}

type StagedChain struct {
	Chain
	ShortName                    string             `toml:"-"`
	Superchain                   Superchain         `toml:"superchain"`
	BaseFeeVaultRecipient        ChecksummedAddress `toml:"base_fee_vault_recipient"`
	L1FeeVaultRecipient          ChecksummedAddress `toml:"l1_fee_vault_recipient"`
	SequencerFeeVaultRecipient   ChecksummedAddress `toml:"sequencer_fee_vault_recipient"`
	DeploymentTxHash             *common.Hash       `toml:"deployment_tx_hash"`
	DeploymentL1ContractsVersion *artifacts.Locator `toml:"deployment_l1_contracts_version"`
	DeploymentL2ContractsVersion *artifacts.Locator `toml:"deployment_l2_contracts_version"`
}

type Chain struct {
	Name                 string              `toml:"name"`
	PublicRPC            string              `toml:"public_rpc"`
	SequencerRPC         string              `toml:"sequencer_rpc"`
	Explorer             string              `toml:"explorer"`
	SuperchainLevel      SuperchainLevel     `toml:"superchain_level"`
	GovernedByOptimism   bool                `toml:"governed_by_optimism"`
	SuperchainTime       *uint64             `toml:"superchain_time"`
	DataAvailabilityType string              `toml:"data_availability_type"`
	ChainID              uint64              `toml:"chain_id"`
	BatchInboxAddr       *ChecksummedAddress `toml:"batch_inbox_addr"`
	BlockTime            uint64              `toml:"block_time"`
	SeqWindowSize        uint64              `toml:"seq_window_size"`
	MaxSequencerDrift    uint64              `toml:"max_sequencer_drift"`
	GasPayingToken       *ChecksummedAddress `toml:"gas_paying_token,omitempty"`
	Hardforks            Hardforks           `toml:"hardforks"`
	Optimism             Optimism            `toml:"optimism"`
	AltDA                *AltDA              `toml:"alt_da"`
	Genesis              Genesis             `toml:"genesis"`
	Roles                Roles               `toml:"roles"`
	Addresses            Addresses           `toml:"addresses"`
}

func (c Chain) ChainListEntry(superchain Superchain, shortName string) ChainListEntry {
	return ChainListEntry{
		Name:                 c.Name,
		Identifier:           fmt.Sprintf("%s/%s", superchain, shortName),
		ChainID:              c.ChainID,
		RPC:                  []string{c.PublicRPC},
		Explorers:            []string{c.Explorer},
		SuperchainLevel:      c.SuperchainLevel,
		GovernedByOptimism:   c.GovernedByOptimism,
		DataAvailabilityType: c.DataAvailabilityType,
		Parent: ChainListEntryParent{
			Type:  "L2",
			Chain: superchain,
		},
		GasPayingToken: c.GasPayingToken,
	}
}

type Hardforks struct {
	CanyonTime   *HardforkTime `toml:"canyon_time"`
	DeltaTime    *HardforkTime `toml:"delta_time"`
	EcotoneTime  *HardforkTime `toml:"ecotone_time"`
	FjordTime    *HardforkTime `toml:"fjord_time"`
	GraniteTime  *HardforkTime `toml:"granite_time"`
	HoloceneTime *HardforkTime `toml:"holocene_time"`
	IsthmusTime  *HardforkTime `toml:"isthmus_time"`
}

type Genesis struct {
	L2Time       uint64       `toml:"l2_time"`
	L1           GenesisRef   `toml:"l1"`
	L2           GenesisRef   `toml:"l2"`
	SystemConfig SystemConfig `toml:"system_config"`
}

type GenesisRef struct {
	Hash   common.Hash `toml:"hash"`
	Number uint64      `toml:"number"`
}

type SystemConfig struct {
	BatcherAddr       ChecksummedAddress `json:"batcherAddr" toml:"batcherAddress"`
	Overhead          common.Hash        `json:"overhead" toml:"overhead"`
	Scalar            common.Hash        `json:"scalar" toml:"scalar"`
	GasLimit          uint64             `json:"gasLimit" toml:"gasLimit"`
	BaseFeeScalar     *uint64            `json:"baseFeeScalar,omitempty" toml:"baseFeeScalar,omitempty"`
	BlobBaseFeeScalar *uint64            `json:"blobBaseFeeScalar,omitempty" toml:"blobBaseFeeScalar,omitempty"`
}

type AltDA struct {
	DaChallengeContractAddress ChecksummedAddress `toml:"da_challenge_contract_address"`
	DaChallengeWindow          uint64             `toml:"da_challenge_window"`
	DaResolveWindow            uint64             `toml:"da_resolve_window"`
	DaCommitmentType           string             `toml:"da_commitment_type"`
}

type Optimism struct {
	EIP1559Elasticity        uint64 `toml:"eip1559_elasticity"`
	EIP1559Denominator       uint64 `toml:"eip1559_denominator"`
	EIP1559DenominatorCanyon uint64 `toml:"eip1559_denominator_canyon"`
}

type Roles struct {
	SystemConfigOwner *ChecksummedAddress `json:"SystemConfigOwner" toml:"SystemConfigOwner"`
	ProxyAdminOwner   *ChecksummedAddress `json:"ProxyAdminOwner" toml:"ProxyAdminOwner"`
	Guardian          *ChecksummedAddress `json:"Guardian" toml:"Guardian"`
	Challenger        *ChecksummedAddress `json:"Challenger" toml:"Challenger"`
	Proposer          *ChecksummedAddress `json:"Proposer,omitempty" toml:"Proposer,omitempty"`
	UnsafeBlockSigner *ChecksummedAddress `json:"UnsafeBlockSigner,omitempty" toml:"UnsafeBlockSigner,omitempty"`
	BatchSubmitter    *ChecksummedAddress `json:"BatchSubmitter" toml:"BatchSubmitter"`
}

type Addresses struct {
	AddressManager                    *ChecksummedAddress `toml:"AddressManager,omitempty" json:"AddressManager,omitempty"`
	L1CrossDomainMessengerProxy       *ChecksummedAddress `toml:"L1CrossDomainMessengerProxy,omitempty" json:"L1CrossDomainMessengerProxy,omitempty"`
	L1ERC721BridgeProxy               *ChecksummedAddress `toml:"L1ERC721BridgeProxy,omitempty" json:"L1ERC721BridgeProxy,omitempty"`
	L1StandardBridgeProxy             *ChecksummedAddress `toml:"L1StandardBridgeProxy,omitempty" json:"L1StandardBridgeProxy,omitempty"`
	L2OutputOracleProxy               *ChecksummedAddress `toml:"L2OutputOracleProxy,omitempty" json:"L2OutputOracleProxy,omitempty"`
	OptimismMintableERC20FactoryProxy *ChecksummedAddress `toml:"OptimismMintableERC20FactoryProxy,omitempty" json:"OptimismMintableERC20FactoryProxy,omitempty"`
	OptimismPortalProxy               *ChecksummedAddress `toml:"OptimismPortalProxy,omitempty" json:"OptimismPortalProxy,omitempty"`
	SystemConfigProxy                 *ChecksummedAddress `toml:"SystemConfigProxy,omitempty" json:"SystemConfigProxy,omitempty"`
	ProxyAdmin                        *ChecksummedAddress `toml:"ProxyAdmin,omitempty" json:"ProxyAdmin,omitempty"`
	SuperchainConfig                  *ChecksummedAddress `toml:"SuperchainConfig,omitempty" json:"SuperchainConfig,omitempty"`
	AnchorStateRegistryProxy          *ChecksummedAddress `toml:"AnchorStateRegistryProxy,omitempty" json:"AnchorStateRegistryProxy,omitempty"`
	DelayedWETHProxy                  *ChecksummedAddress `toml:"DelayedWETHProxy,omitempty" json:"DelayedWETHProxy,omitempty"`
	DisputeGameFactoryProxy           *ChecksummedAddress `toml:"DisputeGameFactoryProxy,omitempty" json:"DisputeGameFactoryProxy,omitempty"`
	FaultDisputeGame                  *ChecksummedAddress `toml:"FaultDisputeGame,omitempty" json:"FaultDisputeGame,omitempty"`
	MIPS                              *ChecksummedAddress `toml:"MIPS,omitempty" json:"MIPS,omitempty"`
	PermissionedDisputeGame           *ChecksummedAddress `toml:"PermissionedDisputeGame,omitempty" json:"PermissionedDisputeGame,omitempty"`
	PreimageOracle                    *ChecksummedAddress `toml:"PreimageOracle,omitempty" json:"PreimageOracle,omitempty"`
	DAChallengeAddress                *ChecksummedAddress `toml:"DAChallengeAddress,omitempty" json:"DAChallengeAddress,omitempty"`
}

type AddressesJSON jsonutil.LazySortedJsonMap[string, *AddressesWithRoles]

type AddressesWithRoles struct {
	Addresses
	Roles
}

func (a AddressesWithRoles) MarshalJSON() ([]byte, error) {
	// Create a map to hold all fields
	allFields := make(map[string]*ChecksummedAddress)

	for _, embed := range []any{a.Addresses, a.Roles} {
		val := reflect.ValueOf(embed)
		typ := reflect.TypeOf(embed)
		for i := 0; i < val.NumField(); i++ {
			field := val.Field(i)

			jsonTag := typ.Field(i).Tag.Get("json")
			tagSplit := strings.Split(jsonTag, ",")
			fieldName := tagSplit[0]
			var omitEmpty bool
			if fieldName == "" {
				fieldName = typ.Field(i).Name
			}
			if len(tagSplit) > 1 {
				omitEmpty = tagSplit[1] == "omitempty"
			}

			if !omitEmpty && field.IsNil() {
				allFields[fieldName] = nil
			} else if !field.IsNil() {
				allFields[fieldName] = field.Interface().(*ChecksummedAddress)
			}
		}
	}

	return json.Marshal(allFields)
}
