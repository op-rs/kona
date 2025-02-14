package main

import (
	"fmt"
	"os"
	"path"

	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/inspect"
	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/state"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/manage"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/output"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
	"github.com/urfave/cli/v2"
)

var (
	StateFilename = &cli.StringFlag{
		Name:      "state-filename",
		Usage:     "Filename of an op-deployer state file.",
		Required:  true,
		TakesFile: true,
	}
	Shortname = &cli.StringFlag{
		Name:     "shortname",
		Usage:    "Shortname of the chain.",
		Required: true,
		Value:    "newchain",
	}
)

func main() {
	app := &cli.App{
		Name:  "create-config",
		Usage: "Turns a state file into a chain config in the stating directory.",
		Flags: []cli.Flag{
			StateFilename,
			Shortname,
		},
		Action: action,
	}
	if err := app.Run(os.Args); err != nil {
		output.WriteStderr("%v", err)
		os.Exit(1)
	}
}

func action(cliCtx *cli.Context) error {
	wd, err := paths.FindRepoRoot()
	if err != nil {
		return fmt.Errorf("failed to get working directory: %w", err)
	}

	statePath := cliCtx.String(StateFilename.Name)
	output.WriteStderr("reading state file from %s", statePath)
	var st state.State
	if err := paths.ReadJSONFile(statePath, &st); err != nil {
		return fmt.Errorf("failed to read state file: %w", err)
	}

	output.WriteOK("inflating chain config")
	cfg, err := manage.InflateChainConfig(&st)
	if err != nil {
		return fmt.Errorf("failed to inflate chain config: %w", err)
	}
	cfg.ShortName = cliCtx.String(Shortname.Name)

	output.WriteOK("reading genesis")
	genesis, _, err := inspect.GenesisAndRollup(&st, st.AppliedIntent.Chains[0].ID)
	if err != nil {
		return fmt.Errorf("failed to get genesis: %w", err)
	}

	stagingDir := paths.StagingDir(wd)

	output.WriteOK("writing chain config")
	if err := paths.WriteTOMLFile(path.Join(stagingDir, cfg.ShortName+".toml"), cfg); err != nil {
		return fmt.Errorf("failed to write chain config: %w", err)
	}

	output.WriteOK("writing genesis")
	if err := manage.WriteGenesis(wd, path.Join(stagingDir, cfg.ShortName+".json.zst"), genesis); err != nil {
		return fmt.Errorf("failed to write genesis: %w", err)
	}

	output.WriteOK("done")
	return nil
}
