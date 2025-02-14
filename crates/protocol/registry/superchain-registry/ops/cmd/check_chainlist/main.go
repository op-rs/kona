package main

import (
	"errors"
	"fmt"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/manage"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/output"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
)

func main() {
	if err := mainErr(); err != nil {
		output.WriteNotOK("%v\n", err)
	}
}

func mainErr() error {
	wd, err := paths.FindRepoRoot()
	if err != nil {
		return fmt.Errorf("failed to get working directory: %w", err)
	}

	chainCfg, err := manage.StagedChainConfig(wd)
	if errors.Is(err, manage.ErrNoStagedConfig) {
		output.WriteOK("no staged chain config found, exiting")
		return nil
	}
	if err != nil {
		return fmt.Errorf("failed to get staged chain config: %w", err)
	}

	output.WriteOK("validating uniqueness for chain ID %d and short name %s", chainCfg.ChainID, chainCfg.ShortName)

	globalChainData, err := manage.FetchGlobalChainIDs()
	if err != nil {
		return fmt.Errorf("failed to fetch global chain IDs: %w", err)
	}
	output.WriteOK("fetched global chain IDs")

	entry, ok := globalChainData[chainCfg.ChainID]
	if !ok {
		return fmt.Errorf("chain ID %d not found in global chain data", chainCfg.ChainID)
	}
	output.WriteOK("chain ID %d found in global chain data", chainCfg.ChainID)

	if entry.ShortName != chainCfg.ShortName {
		return fmt.Errorf("short name %s does not match global chain data short name %s", chainCfg.ShortName, entry.ShortName)
	}
	output.WriteOK("short name %s matches global chain data short name %s", chainCfg.ShortName, entry.ShortName)

	if entry.Name != chainCfg.Name {
		return fmt.Errorf("name %s does not match global chain data name %s", chainCfg.Name, entry.Name)
	}
	output.WriteOK("name %s matches global chain data name %s", chainCfg.Name, entry.Name)
	output.WriteOK("chainlist check passed")
	return nil
}
