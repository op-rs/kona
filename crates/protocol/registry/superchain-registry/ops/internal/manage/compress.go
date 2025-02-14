package manage

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/fs"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
	"github.com/ethereum/go-ethereum/core"
	"github.com/klauspost/compress/zstd"
)

func WriteSuperchainGenesis(rootP string, superchain config.Superchain, shortName string, gen *core.Genesis) error {
	genPath := paths.GenesisFile(rootP, superchain, shortName)
	exists, err := fs.FileExists(genPath)
	if err != nil {
		return fmt.Errorf("failed to check if genesis exists: %w", err)
	}
	if exists {
		return fmt.Errorf("genesis already exists: %s", genPath)
	}

	return WriteGenesis(rootP, genPath, gen)
}

func WriteGenesis(rootP string, genPath string, gen *core.Genesis) error {
	dictPath := path.Join(paths.ExtraDir(rootP), "dictionary")
	dict, err := os.ReadFile(dictPath)
	if err != nil {
		return fmt.Errorf("failed to read dictionary: %w", err)
	}

	buf := new(bytes.Buffer)
	zr, err := zstd.NewWriter(buf, zstd.WithEncoderDict(dict))
	if err != nil {
		return fmt.Errorf("failed to create zstd writer: %w", err)
	}
	if err := json.NewEncoder(zr).Encode(gen); err != nil {
		return fmt.Errorf("failed to encode genesis: %w", err)
	}
	if err := zr.Close(); err != nil {
		return fmt.Errorf("failed to close zstd writer: %w", err)
	}

	if err := fs.AtomicWrite(genPath, 0o755, buf.Bytes()); err != nil {
		return fmt.Errorf("failed to write genesis: %w", err)
	}

	return nil
}

func ReadSuperchainGenesis(rootP string, superchain config.Superchain, shortName string) (*core.Genesis, error) {
	genPath := paths.GenesisFile(rootP, superchain, shortName)
	exists, err := fs.FileExists(genPath)
	if err != nil {
		return nil, fmt.Errorf("failed to check if genesis exists: %w", err)
	}
	if !exists {
		return nil, fmt.Errorf("genesis does not exist: %s", genPath)
	}

	return ReadGenesis(rootP, genPath)
}

func ReadGenesis(rootP string, genPath string) (*core.Genesis, error) {
	dictPath := path.Join(paths.ExtraDir(rootP), "dictionary")
	dict, err := os.ReadFile(dictPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read dictionary: %w", err)
	}

	genF, err := os.Open(genPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open genesis: %w", err)
	}
	defer genF.Close()

	zr, err := zstd.NewReader(genF, zstd.WithDecoderDicts(dict))
	if err != nil {
		return nil, fmt.Errorf("failed to create zstd reader: %w", err)
	}
	defer zr.Close()

	var gen core.Genesis
	if err := json.NewDecoder(zr).Decode(&gen); err != nil {
		return nil, fmt.Errorf("failed to decode genesis: %w", err)
	}

	return &gen, nil
}
