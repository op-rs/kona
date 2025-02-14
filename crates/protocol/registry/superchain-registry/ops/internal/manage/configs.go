package manage

import (
	"fmt"
	"os"

	"github.com/BurntSushi/toml"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/fs"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
)

func WriteChainConfig(rootP string, in *config.StagedChain) error {
	fname := paths.ChainConfig(rootP, in.Superchain, in.ShortName)
	exists, err := fs.FileExists(fname)
	if err != nil {
		return fmt.Errorf("failed to check if file exists: %w", err)
	}
	if exists {
		return fmt.Errorf("file already exists: %s", fname)
	}

	data, err := toml.Marshal(in.Chain)
	if err != nil {
		return fmt.Errorf("failed to marshal toml: %w", err)
	}

	if err := fs.AtomicWrite(fname, 0o755, data); err != nil {
		return fmt.Errorf("failed to write config: %w", err)
	}
	return nil
}

func ReadChainConfig(rootP string, superchain config.Superchain, shortName string) (*config.Chain, error) {
	fname := paths.ChainConfig(rootP, superchain, shortName)
	exists, err := fs.FileExists(fname)
	if err != nil {
		return nil, fmt.Errorf("failed to check if file exists: %w", err)
	}
	if !exists {
		return nil, fmt.Errorf("file does not exist: %s", fname)
	}

	data, err := os.ReadFile(fname)
	if err != nil {
		return nil, fmt.Errorf("failed to read file: %w", err)
	}

	var out config.Chain
	if err := toml.Unmarshal(data, &out); err != nil {
		return nil, fmt.Errorf("failed to unmarshal toml: %w", err)
	}

	return &out, nil
}
