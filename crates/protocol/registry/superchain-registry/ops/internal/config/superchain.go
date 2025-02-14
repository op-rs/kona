package config

import "fmt"

type Superchain string

const (
	MainnetSuperchain     Superchain = "mainnet"
	SepoliaSuperchain     Superchain = "sepolia"
	SepoliaDev0Superchain Superchain = "sepolia-dev-0"
)

func ParseSuperchain(in string) (Superchain, error) {
	switch Superchain(in) {
	case MainnetSuperchain, SepoliaSuperchain, SepoliaDev0Superchain:
		return Superchain(in), nil
	default:
		return "", fmt.Errorf("unknown superchain: '%s'", in)
	}
}

func MustParseSuperchain(in string) Superchain {
	sup, err := ParseSuperchain(in)
	if err != nil {
		panic(err)
	}
	return sup
}

func (s *Superchain) UnmarshalText(text []byte) error {
	sup, err := ParseSuperchain(string(text))
	if err != nil {
		return err
	}
	*s = sup
	return nil
}

type SuperchainDefinition struct {
	Name                   string
	ProtocolVersionsAddr   *ChecksummedAddress `toml:"protocol_versions_addr"`
	SuperchainConfigAddr   *ChecksummedAddress `toml:"superchain_config_addr"`
	OPContractsManagerAddr *ChecksummedAddress `toml:"op_contracts_manager_addr"`
	Hardforks              Hardforks           `toml:"hardforks"`
	L1                     SuperchainL1        `toml:"l1"`
}

type SuperchainL1 struct {
	ChainID   uint64
	PublicRPC string `toml:"public_rpc"`
	Explorer  string `toml:"explorer"`
}
