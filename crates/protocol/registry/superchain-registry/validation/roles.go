package validation

import (
	_ "embed"
	"fmt"

	"github.com/BurntSushi/toml"
)

type RolesConfig struct {
	Guardian              Address `toml:"guardian"`
	Challenger            Address `toml:"challenger"`
	L1ProxyAdminOwner     Address `toml:"l1ProxyAdminOwner"`
	L2ProxyAdminOwner     Address `toml:"l2ProxyAdminOwner"`
	ProtocolVersionsOwner Address `toml:"protocolVersionsOwner"`
}

//go:embed standard/standard-config-roles-mainnet.toml
var standardConfigRolesMainnetToml []byte

//go:embed standard/standard-config-roles-sepolia.toml
var standardConfigRolesSepoliaToml []byte

var (
	StandardConfigRolesMainnet RolesConfig
	StandardConfigRolesSepolia RolesConfig
)

func init() {
	if err := toml.Unmarshal(standardConfigRolesMainnetToml, &StandardConfigRolesMainnet); err != nil {
		panic(fmt.Errorf("failed to unmarshal mainnet standard config roles: %w", err))
	}
	if err := toml.Unmarshal(standardConfigRolesSepoliaToml, &StandardConfigRolesSepolia); err != nil {
		panic(fmt.Errorf("failed to unmarshal sepolia standard config roles: %w", err))
	}
}
