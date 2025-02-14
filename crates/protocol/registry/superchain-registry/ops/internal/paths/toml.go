package paths

import (
	"compress/gzip"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/fs"
)

func ReadTOMLFile(p string, out any) error {
	data, err := os.ReadFile(p)
	if err != nil {
		return fmt.Errorf("failed to read TOML file: %w", err)
	}

	if err := toml.Unmarshal(data, out); err != nil {
		return fmt.Errorf("failed to unmarshal TOML: %w", err)
	}

	return nil
}

func ReadJSONFile(p string, out any) error {
	f, err := os.Open(p)
	if err != nil {
		return fmt.Errorf("failed to open JSON file: %w", err)
	}
	defer f.Close()

	var r io.Reader
	if filepath.Ext(p) == ".gz" {
		gzr, err := gzip.NewReader(f)
		if err != nil {
			return fmt.Errorf("failed to create gzip reader: %w", err)
		}
		defer gzr.Close()
		r = gzr
	} else {
		r = f
	}

	if err := json.NewDecoder(r).Decode(out); err != nil {
		return fmt.Errorf("failed to unmarshal JSON: %w", err)
	}

	return nil
}

func WriteTOMLFile(p string, in any) error {
	data, err := toml.Marshal(in)
	if err != nil {
		return fmt.Errorf("failed to marshal JSON: %w", err)
	}

	if err := fs.AtomicWrite(p, 0o644, data); err != nil {
		return fmt.Errorf("failed to write JSON file: %w", err)
	}

	return nil
}
