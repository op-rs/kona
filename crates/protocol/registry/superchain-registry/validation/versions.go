package validation

import (
	_ "embed"
	"fmt"
	"slices"

	"github.com/BurntSushi/toml"
)

type Semver string

const (
	Semver130 Semver = "op-contracts/v1.3.0"
	Semver140 Semver = "op-contracts/v1.4.0"
	Semver160 Semver = "op-contracts/v1.6.0"
	Semver170 Semver = "op-contracts/v1.7.0-beta.1+l2-contracts"
	Semver180 Semver = "op-contracts/v1.8.0-rc.4"
)

var validSemvers = []Semver{
	Semver130,
	Semver140,
	Semver160,
	Semver170,
	Semver180,
}

func IsValidContractSemver(s string) bool {
	return slices.Contains(validSemvers, Semver(s))
}

// ContractData represents the version and address information for a contract
type ContractData struct {
	Version               string   `toml:"version"`
	Address               *Address `toml:"address,omitempty"`
	ImplementationAddress *Address `toml:"implementation_address,omitempty"`
}

// VersionConfig represents all contracts for a specific release version
type VersionConfig struct {
	OptimismPortal               *ContractData `toml:"optimism_portal,omitempty"`
	SystemConfig                 *ContractData `toml:"system_config,omitempty"`
	AnchorStateRegistry          *ContractData `toml:"anchor_state_registry,omitempty"`
	DelayedWeth                  *ContractData `toml:"delayed_weth,omitempty"`
	DisputeGameFactory           *ContractData `toml:"dispute_game_factory,omitempty"`
	FaultDisputeGame             *ContractData `toml:"fault_dispute_game,omitempty"`
	PermissionedDisputeGame      *ContractData `toml:"permissioned_dispute_game,omitempty"`
	Mips                         *ContractData `toml:"mips,omitempty"`
	PreimageOracle               *ContractData `toml:"preimage_oracle,omitempty"`
	L1CrossDomainMessenger       *ContractData `toml:"l1_cross_domain_messenger,omitempty"`
	L1ERC721Bridge               *ContractData `toml:"l1_erc721_bridge,omitempty"`
	L1StandardBridge             *ContractData `toml:"l1_standard_bridge,omitempty"`
	L2OutputOracle               *ContractData `toml:"l2_output_oracle,omitempty"`
	OptimismMintableERC20Factory *ContractData `toml:"optimism_mintable_erc20_factory,omitempty"`
}

// Versions maps release tags to their contract configurations
type Versions map[Semver]VersionConfig

//go:embed standard/standard-versions-mainnet.toml
var standardVersionsMainnetToml []byte

//go:embed standard/standard-versions-sepolia.toml
var standardVersionsSepoliaToml []byte

var (
	StandardVersionsMainnet Versions
	StandardVersionsSepolia Versions
)

func init() {
	if err := toml.Unmarshal(standardVersionsMainnetToml, &StandardVersionsMainnet); err != nil {
		panic(fmt.Errorf("failed to unmarshal mainnet standard versions: %w", err))
	}
	if err := toml.Unmarshal(standardVersionsSepoliaToml, &StandardVersionsSepolia); err != nil {
		panic(fmt.Errorf("failed to unmarshal sepolia standard versions: %w", err))
	}
}
