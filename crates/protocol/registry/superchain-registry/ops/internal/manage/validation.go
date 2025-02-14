package manage

import (
	"encoding/json"
	"fmt"
	"math/big"
	"net/http"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core"
	"github.com/ethereum/go-ethereum/params"
)

const chainListUrl = "https://chainid.network/chains_mini.json"

var (
	ErrDuplicateChainID    = fmt.Errorf("duplicate chain ID")
	ErrDuplicateShortName  = fmt.Errorf("duplicate short name")
	ErrGenesisHashMismatch = fmt.Errorf("genesis hash mismatch")
)

type GlobalChainIDs struct {
	ChainIDs   map[uint64]bool
	ShortNames map[string]bool
}

func ValidateUniqueness(
	in *config.StagedChain,
	chains []DiskChainConfig,
) error {
	for _, chain := range chains {
		if chain.Config.ChainID == in.ChainID {
			return ErrDuplicateChainID
		}

		if chain.ShortName == in.ShortName {
			return ErrDuplicateShortName
		}
	}
	return nil
}

type ChainEntry struct {
	ChainID   uint64 `json:"chainId"`
	Name      string `json:"name"`
	ShortName string `json:"shortName"`
}

func FetchGlobalChainIDs() (map[uint64]ChainEntry, error) {
	req, err := http.NewRequest(http.MethodGet, chainListUrl, nil)
	if err != nil {
		return nil, err
	}

	req.Header.Set("User-Agent", "optimism-superchain-registry-validation")

	res, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()

	var entries []ChainEntry
	if err := json.NewDecoder(res.Body).Decode(&entries); err != nil {
		return nil, err
	}

	out := make(map[uint64]ChainEntry)
	for _, entry := range entries {
		out[entry.ChainID] = entry
	}
	return out, nil
}

func ValidateGenesisIntegrity(cfg *config.Chain, genesis *core.Genesis) error {
	genesisActivation := uint64(0)
	out := &params.ChainConfig{
		ChainID:                 new(big.Int).SetUint64(cfg.ChainID),
		HomesteadBlock:          common.Big0,
		DAOForkBlock:            nil,
		DAOForkSupport:          false,
		EIP150Block:             common.Big0,
		EIP155Block:             common.Big0,
		EIP158Block:             common.Big0,
		ByzantiumBlock:          common.Big0,
		ConstantinopleBlock:     common.Big0,
		PetersburgBlock:         common.Big0,
		IstanbulBlock:           common.Big0,
		MuirGlacierBlock:        common.Big0,
		BerlinBlock:             common.Big0,
		LondonBlock:             common.Big0,
		ArrowGlacierBlock:       common.Big0,
		GrayGlacierBlock:        common.Big0,
		MergeNetsplitBlock:      common.Big0,
		ShanghaiTime:            cfg.Hardforks.CanyonTime.U64Ptr(),  // Shanghai activates with Canyon
		CancunTime:              cfg.Hardforks.EcotoneTime.U64Ptr(), // Cancun activates with Ecotone
		PragueTime:              nil,
		BedrockBlock:            common.Big0,
		RegolithTime:            &genesisActivation,
		CanyonTime:              cfg.Hardforks.CanyonTime.U64Ptr(),
		EcotoneTime:             cfg.Hardforks.EcotoneTime.U64Ptr(),
		FjordTime:               cfg.Hardforks.FjordTime.U64Ptr(),
		GraniteTime:             cfg.Hardforks.GraniteTime.U64Ptr(),
		HoloceneTime:            cfg.Hardforks.HoloceneTime.U64Ptr(),
		TerminalTotalDifficulty: common.Big0,
		Ethash:                  nil,
		Clique:                  nil,
	}

	out.Optimism = &params.OptimismConfig{
		EIP1559Elasticity:  cfg.Optimism.EIP1559Elasticity,
		EIP1559Denominator: cfg.Optimism.EIP1559Denominator,
	}

	if cfg.Optimism.EIP1559DenominatorCanyon != 0 {
		out.Optimism.EIP1559DenominatorCanyon = &cfg.Optimism.EIP1559DenominatorCanyon
	}

	genCopy := &core.Genesis{
		Config:        out,
		Nonce:         genesis.Nonce,
		Timestamp:     genesis.Timestamp,
		ExtraData:     genesis.ExtraData,
		GasLimit:      genesis.GasLimit,
		Difficulty:    genesis.Difficulty,
		Mixhash:       genesis.Mixhash,
		Coinbase:      genesis.Coinbase,
		Alloc:         genesis.Alloc,
		Number:        genesis.Number,
		GasUsed:       genesis.GasUsed,
		ParentHash:    genesis.ParentHash,
		BaseFee:       genesis.BaseFee,
		ExcessBlobGas: genesis.ExcessBlobGas,
		BlobGasUsed:   genesis.BlobGasUsed,
		StateHash:     genesis.StateHash,
	}
	block := genCopy.ToBlock()

	if block.Hash() != cfg.Genesis.L2.Hash {
		return fmt.Errorf("%w: expected %s, got %s", ErrGenesisHashMismatch, cfg.Genesis.L2.Hash.Hex(), block.Hash().Hex())
	}

	return nil
}
