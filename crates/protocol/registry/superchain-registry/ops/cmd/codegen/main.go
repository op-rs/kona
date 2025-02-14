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
		output.WriteNotOK("application error: %v", err)
		os.Exit(1)
	}
}

func mainErr() error {
	wd, err := paths.FindRepoRoot()
	if err != nil {
		return fmt.Errorf("failed to get working directory: %w", err)
	}

	if err := manage.GenAllCode(wd); err != nil {
		return fmt.Errorf("error generating code: %w", err)
	}

	return nil
}
