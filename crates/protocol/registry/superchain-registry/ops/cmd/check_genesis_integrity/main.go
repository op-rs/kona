package main

import (
	"fmt"
	"os"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/manage"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/output"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
)

func main() {
	if err := mainErr(); err != nil {
		output.WriteStderr("%v\n", err)
		os.Exit(1)
	}
}

func mainErr() error {
	wd, err := paths.FindRepoRoot()
	if err != nil {
		return fmt.Errorf("failed to get working directory: %w", err)
	}

	superchains, err := paths.Superchains(wd)
	if err != nil {
		return fmt.Errorf("error getting superchains: %w", err)
	}

	var integrityCheckFailed bool
	for _, superchain := range superchains {
		cfgs, err := manage.CollectChainConfigs(paths.SuperchainDir(wd, superchain))
		if err != nil {
			return fmt.Errorf("error collecting chain configs: %w", err)
		}

		for _, cfg := range cfgs {
			if cfg.ShortName == "op" {
				output.WriteWarn("skipping op %s - chain was migrated from a legacy state", superchain)
				continue
			}

			genesis, err := manage.ReadSuperchainGenesis(wd, superchain, cfg.ShortName)
			if err != nil {
				output.WriteNotOK("error decompressing genesis for %s/%s: %v", superchain, cfg.ShortName, err)
				integrityCheckFailed = true
				continue
			}

			if err := manage.ValidateGenesisIntegrity(cfg.Config, genesis); err != nil {
				integrityCheckFailed = true
				output.WriteNotOK("genesis integrity check failed for %s: %v", cfg.ShortName, err)
			} else {
				output.WriteOK("genesis integrity check passed for %s/%s", superchain, cfg.ShortName)
			}
		}
	}

	if integrityCheckFailed {
		return fmt.Errorf("one or more genesis integrity checks failed - see logs")
	}

	return nil
}
