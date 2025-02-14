package main

import (
	"errors"
	"fmt"
	"os"
	"path"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/manage"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/output"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
	"github.com/urfave/cli/v2"
)

var (
	FlagCheck = &cli.BoolFlag{
		Name:  "check",
		Usage: "Perform validation only.",
	}
	FlagPreserveInput = &cli.BoolFlag{
		Name:  "preserve-input",
		Usage: "Skip cleanup of staging directory.",
	}
)

func main() {
	app := &cli.App{
		Name:  "sync-staging",
		Usage: "Syncs a staging directory with the superchain registry.",
		Flags: []cli.Flag{
			FlagCheck,
			FlagPreserveInput,
		},
		Action: action,
	}
	if err := app.Run(os.Args); err != nil {
		output.WriteStderr("%v", err)
		os.Exit(1)
	}
}

func action(cliCtx *cli.Context) error {
	check := cliCtx.Bool(FlagCheck.Name)
	noCleanup := cliCtx.Bool(FlagPreserveInput.Name)
	wd, err := paths.FindRepoRoot()
	if err != nil {
		return fmt.Errorf("failed to get working directory: %w", err)
	}

	stagingDir := paths.StagingDir(wd)

	chainCfg, err := manage.StagedChainConfig(wd)
	if errors.Is(err, manage.ErrNoStagedConfig) {
		output.WriteOK("no staged chain config found, exiting")
		return nil
	}
	if err != nil {
		return fmt.Errorf("failed to get staged chain config: %w", err)
	}

	genesisFilename := path.Join(stagingDir, chainCfg.ShortName+".json.zst")
	genesis, err := manage.ReadGenesis(wd, genesisFilename)
	if err != nil {
		return fmt.Errorf("failed to read genesis: %w", err)
	}

	superchainPath := paths.SuperchainDir(wd, chainCfg.Superchain)
	chainCfgs, err := manage.CollectChainConfigs(superchainPath)
	if err != nil {
		return fmt.Errorf("failed to collect chain configs: %w", err)
	}

	if err := manage.ValidateUniqueness(chainCfg, chainCfgs); err != nil {
		return fmt.Errorf("failed uniqueness check: %w", err)
	}
	output.WriteOK("internal uniqueness check passed")

	if check {
		output.WriteOK("validation successful")
		return nil
	}

	if err := manage.WriteChainConfig(wd, chainCfg); err != nil {
		return fmt.Errorf("failed to write chain config: %w", err)
	}

	output.WriteOK(
		"wrote chain config %s.toml to %s superchain",
		chainCfg.ShortName,
		chainCfg.Superchain,
	)

	if err := manage.WriteSuperchainGenesis(wd, chainCfg.Superchain, chainCfg.ShortName, genesis); err != nil {
		return fmt.Errorf("failed to compress genesis: %w", err)
	}

	output.WriteOK("wrote genesis files")

	if err := manage.GenAllCode(wd); err != nil {
		return fmt.Errorf("failed to generate code: %w", err)
	}

	if !noCleanup {
		cfgFilename := path.Join(stagingDir, chainCfg.ShortName+".toml")
		if err := os.Remove(cfgFilename); err != nil {
			output.WriteNotOK("failed to remove %s: %v", cfgFilename, err)
		}
		if err := os.Remove(genesisFilename); err != nil {
			output.WriteNotOK("failed to remove %s: %v", genesisFilename, err)
		}

		output.WriteOK("cleaned up staging directory")
	}

	return nil
}
