package config

type ChainListEntry struct {
	Name                 string               `json:"name" toml:"name"`
	Identifier           string               `json:"identifier" toml:"identifier"`
	ChainID              uint64               `json:"chainId" toml:"chain_id"`
	RPC                  []string             `json:"rpc" toml:"rpc"`
	Explorers            []string             `json:"explorers" toml:"explorers"`
	SuperchainLevel      SuperchainLevel      `json:"superchainLevel" toml:"superchain_level"`
	GovernedByOptimism   bool                 `json:"governedByOptimism" toml:"governed_by_optimism"`
	DataAvailabilityType string               `json:"dataAvailabilityType" toml:"data_availability_type"`
	Parent               ChainListEntryParent `json:"parent" toml:"parent"`
	GasPayingToken       *ChecksummedAddress  `json:"gasPayingToken,omitempty" toml:"gas_paying_token,omitempty"`
}

type ChainListEntryParent struct {
	Type  string     `json:"type" toml:"type"`
	Chain Superchain `json:"chain" toml:"chain"`
}

type ChainListTOML struct {
	Chains []ChainListEntry `toml:"chains"`
}
