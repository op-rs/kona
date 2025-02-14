package manage

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"sync"

	"github.com/BurntSushi/toml"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/util"
)

const CollectorConcurrency = 8

type DiskChainConfig struct {
	ShortName string
	Filepath  string
	Config    *config.Chain
}

func CollectChainConfigs(p string) ([]DiskChainConfig, error) {
	var files []string

	err := filepath.Walk(p, func(fp string, info fs.FileInfo, err error) error {
		basePath := filepath.Base(fp)
		ext := filepath.Ext(basePath)
		if info.IsDir() || basePath == "superchain.toml" || ext != ".toml" {
			return nil
		}

		files = append(files, fp)
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("failed to walk directory: %w", err)
	}

	filesCh := make(chan string)
	firstErr := new(util.OnceValue[error])
	var wg sync.WaitGroup
	var mtx sync.Mutex
	var out []DiskChainConfig

	worker := func() {
		defer wg.Done()
		for file := range filesCh {
			data, err := os.ReadFile(file)
			if err != nil {
				firstErr.Set(fmt.Errorf("failed to read file %s: %w", file, err))
				return
			}

			basename := filepath.Base(file)
			var chain config.Chain
			if err := toml.Unmarshal(data, &chain); err != nil {
				firstErr.Set(fmt.Errorf("failed to unmarshal toml %s: %w", basename, err))
				return
			}

			mtx.Lock()
			out = append(out, DiskChainConfig{
				ShortName: strings.TrimSuffix(basename, ".toml"),
				Filepath:  file,
				Config:    &chain,
			})
			mtx.Unlock()
		}
	}

	for i := 0; i < CollectorConcurrency; i++ {
		wg.Add(1)
		go worker()
	}

	for _, file := range files {
		filesCh <- file
	}
	close(filesCh)
	wg.Wait()

	if err := firstErr.V; err != nil {
		return nil, fmt.Errorf("error collecting configs: %w", err)
	}

	slices.SortFunc(out, func(a, b DiskChainConfig) int {
		return strings.Compare(a.Config.Name, b.Config.Name)
	})

	return out, nil
}
