package validation

import (
	_ "embed"
	"fmt"

	"github.com/BurntSushi/toml"
)

type Range [2]int

func (r Range) WithinRange(v int) bool {
	return v >= r[0] && v <= r[1]
}

type RollupConfigParams struct {
	SeqWindowSize Range `toml:"seq_window_size"`
	BlockTime     Range `toml:"block_time"`
}

type OptimismPortal2Params struct {
	ProofMaturityDelaySeconds       Range `toml:"proof_maturity_delay_seconds"`
	DisputeGameFinalityDelaySeconds Range `toml:"dispute_game_finality_delay_seconds"`
	RespectedGameType               int   `toml:"respected_game_type"`
}

type ResourceConfigParams struct {
	MaxResourceLimit            int    `toml:"max_resource_limit"`
	ElasticityMultiplier        int    `toml:"elasticity_multiplier"`
	BaseFeeMaxChangeDenominator int    `toml:"base_fee_max_change_denominator"`
	MinimumBaseFee              int    `toml:"minimum_base_fee"`
	SystemTxMaxGas              int    `toml:"system_tx_max_gas"`
	MaximumBaseFee              string `toml:"maximum_base_fee"`
}

type PreEcotoneGasPriceOracleParams struct {
	Decimals Range `toml:"decimals"`
	Overhead Range `toml:"overhead"`
	Scalar   Range `toml:"scalar"`
}

type EcotoneGasPriceOracleParams struct {
	Decimals          Range `toml:"decimals"`
	BlobBaseFeeScalar Range `toml:"blob_base_fee_scalar"`
	BaseFeeScalar     Range `toml:"base_fee_scalar"`
}

type GasPriceOracleParams struct {
	PreEcotone PreEcotoneGasPriceOracleParams `toml:"pre-ecotone"`
	Ecotone    EcotoneGasPriceOracleParams    `toml:"ecotone"`
}

type SystemConfigParams struct {
	GasLimit Range `toml:"gas_limit"`
}

type FDGParams struct {
	GameType         uint32 `toml:"game_type"`
	MaxGameDepth     uint64 `toml:"max_game_depth"`
	SplitDepth       uint64 `toml:"split_depth"`
	MaxClockDuration uint64 `toml:"max_clock_duration"`
	ClockExtension   uint64 `toml:"clock_extension"`
}

type ProofsParams struct {
	Permissioned   FDGParams `toml:"permissioned"`
	Permissionless FDGParams `toml:"permissionless"`
}

type ConfigParams struct {
	RollupConfig    RollupConfigParams    `toml:"rollup_config"`
	OptimismPortal2 OptimismPortal2Params `toml:"optimism_portal_2"`
	ResourceConfig  ResourceConfigParams  `toml:"resource_config"`
	GasPriceOracle  GasPriceOracleParams  `toml:"gas_price_oracle"`
	SystemConfig    SystemConfigParams    `toml:"system_config"`
	Proofs          ProofsParams          `toml:"proofs"`
}

//go:embed standard/standard-config-params-mainnet.toml
var standardConfigParamsMainnetToml []byte

//go:embed standard/standard-config-params-sepolia.toml
var standardConfigParamsSepoliaToml []byte

var (
	StandardConfigParamsMainnet ConfigParams
	StandardConfigParamsSepolia ConfigParams
)

func init() {
	if err := toml.Unmarshal(standardConfigParamsMainnetToml, &StandardConfigParamsMainnet); err != nil {
		panic(fmt.Errorf("failed to unmarshal mainnet standard config params: %w", err))
	}
	if err := toml.Unmarshal(standardConfigParamsSepoliaToml, &StandardConfigParamsSepolia); err != nil {
		panic(fmt.Errorf("failed to unmarshal sepoliastandard config params: %w", err))
	}
}
