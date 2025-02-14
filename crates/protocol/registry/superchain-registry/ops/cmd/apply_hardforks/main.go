package main

import (
	"fmt"
	"os"
	"path"

	"github.com/BurntSushi/toml"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/fs"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/manage"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/output"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
)

func main() {
	if err := mainErr(); err != nil {
		output.WriteStderr("%v", err)
		os.Exit(1)
	}
}

func mainErr() error {
	wd, err := paths.FindRepoRoot()
	if err != nil {
		return fmt.Errorf("failed to get working directory: %w", err)
	}

	output.WriteStderr("working directory: %s", wd)

	superchainDir := path.Join(wd, "superchain")
	exists, err := fs.DirExists(superchainDir)
	if err != nil {
		return fmt.Errorf("error checking for superchain directory: %w", err)
	}
	if !exists {
		return fmt.Errorf("superchain directory does not exist - check working directory")
	}

	superchains, err := paths.Superchains(wd)
	if err != nil {
		return fmt.Errorf("error getting superchains: %w", err)
	}

	for _, superchain := range superchains {
		if err := processSuperchainDir(wd, superchain); err != nil {
			return fmt.Errorf("error processing superchain %s: %w", superchain, err)
		}
	}

	return nil
}

func processSuperchainDir(wd string, superchain config.Superchain) error {
	superchainCfgPath := paths.SuperchainConfig(wd, superchain)

	var superchainCfg config.SuperchainDefinition
	if err := paths.ReadTOMLFile(superchainCfgPath, &superchainCfg); err != nil {
		return fmt.Errorf("error reading superchain config: %w", err)
	}

	cfgs, err := manage.CollectChainConfigs(paths.SuperchainDir(wd, superchain))
	if err != nil {
		return fmt.Errorf("error collecting chain configs: %w", err)
	}

	for _, cfg := range cfgs {
		chainCfg := cfg.Config

		if err := config.CopyHardforks(
			&superchainCfg.Hardforks,
			&chainCfg.Hardforks,
			chainCfg.SuperchainTime,
			&chainCfg.Genesis.L2Time,
		); err != nil {
			return fmt.Errorf("error copying hardforks: %w", err)
		}

		realCfgData, err := toml.Marshal(chainCfg)
		if err != nil {
			return fmt.Errorf("error marshaling chain config: %w", err)
		}

		if err := fs.AtomicWrite(cfg.Filepath, 0o755, realCfgData); err != nil {
			return fmt.Errorf("error writing chain config: %w", err)
		}

		output.WriteOK("wrote chain config: %s", cfg.Filepath)
	}

	return nil
}
